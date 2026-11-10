//! Restore & Backup center: System Restore, registry backups, and tweak profile export/import.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::types::{now_ms, JobRequest, Shell};
use crate::error::{AppError, AppResult};
use crate::tweaks::model::State;
use crate::tweaks::{Target, Tweaks};
use crate::util::registry::{self, Hive, RegData};

fn err(m: impl Into<String>) -> AppError {
    AppError::Other(m.into())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionStatus {
    /// System Restore is turned on for the system drive.
    pub enabled: bool,
    /// Disabled by a group policy (cannot be turned on here).
    pub disabled_by_policy: bool,
}

pub fn protection_status() -> ProtectionStatus {
    let enabled = matches!(
        registry::read(Hive::LocalMachine, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore", "RPSessionInterval"),
        Some(RegData::Dword(v)) if v > 0
    );
    let disabled_by_policy = matches!(
        registry::read(Hive::LocalMachine, r"SOFTWARE\Policies\Microsoft\Windows NT\SystemRestore", "DisableSR"),
        Some(RegData::Dword(1))
    );
    ProtectionStatus { enabled, disabled_by_policy }
}

/// Lists restore points. Reading them needs administrator rights, so this runs as a job whose
/// output contains one `##RP <seq>|<iso date>|<type>|<description>` line per point.
pub fn list_restore_points() -> JobRequest {
    let script = r###"$points = @(Get-ComputerRestorePoint -ErrorAction Stop)
if (-not $points.Count) { 'No restore points exist yet.' }
foreach ($p in $points) {
    $d = ([string]$p.Description) -replace '[\r\n|]', ' '
    if ($d.Length -gt 120) { $d = $d.Substring(0, 120) }
    "##RP $($p.SequenceNumber)|$($p.ConvertToDateTime($p.CreationTime).ToString('s'))|$($p.RestorePointType)|$d"
}"###;
    let mut r = JobRequest::internal("List restore points", Shell::PowerShell, script);
    r.operation_id = Some("restore.list".into());
    r.requires_admin = true;
    r.modifies_system = false;
    r.cancellable = true;
    r.timeout = Duration::from_secs(120);
    r
}

pub fn restore_to(sequence: u32) -> JobRequest {
    let script = format!(
        "'Windows will restart and roll system files, settings and installed programs back to restore point {sequence}. Your documents are not affected.'\nRestore-Computer -RestorePoint {sequence} -Confirm:$false -ErrorAction Stop"
    );
    let mut r = JobRequest::internal(format!("Restore Windows to restore point {sequence}"), Shell::PowerShell, script);
    r.operation_id = Some("restore.rollback".into());
    r.requires_admin = true;
    r.timeout = Duration::from_secs(600);
    r
}

// ---------------- Registry backups (.reg exports) ----------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegBackup {
    pub file: String,
    pub created: u64,
    pub size_bytes: u64,
    pub keys: Vec<String>,
    pub machine_wide: bool,
}

pub fn registry_backups(dir: &Path) -> Vec<RegBackup> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<RegBackup> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("reg")))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let bytes = std::fs::read(e.path()).ok()?;
            // reg.exe exports UTF-16LE with a BOM.
            let text = if bytes.starts_with(&[0xFF, 0xFE]) {
                String::from_utf16_lossy(&bytes[2..].as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>())
            } else {
                String::from_utf8_lossy(&bytes).into_owned()
            };
            let keys: Vec<String> = text.lines().filter(|l| l.starts_with("[HKEY_") && !l.contains("\\\\")).map(|l| l.trim_matches(['[', ']']).to_string()).collect();
            let top: Vec<String> = keys.iter().filter(|k| !keys.iter().any(|o| o != *k && k.starts_with(&format!("{o}\\")))).cloned().collect();
            Some(RegBackup {
                file: e.file_name().to_string_lossy().into_owned(),
                created: meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64).unwrap_or(0),
                size_bytes: meta.len(),
                machine_wide: keys.iter().any(|k| k.starts_with("HKEY_LOCAL_MACHINE")),
                keys: top,
            })
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

pub fn import_registry_backup(dir: &Path, file: &str) -> AppResult<JobRequest> {
    if file.contains(['\\', '/', ':', '\'']) || !file.to_ascii_lowercase().ends_with(".reg") {
        return Err(err("Invalid backup file name."));
    }
    let b = registry_backups(dir).into_iter().find(|b| b.file == file).ok_or_else(|| err("That backup no longer exists."))?;
    let path = dir.join(file);
    let script = format!(
        "& reg.exe import '{}'\nif ($LASTEXITCODE) {{ exit $LASTEXITCODE }}\n'Restored {} registry key(s) from the backup.'",
        path.display().to_string().replace('\'', "''"),
        b.keys.len()
    );
    let mut r = JobRequest::internal(format!("Restore registry backup {file}"), Shell::PowerShell, script);
    r.operation_id = Some("backup.reg_import".into());
    r.requires_admin = b.machine_wide;
    r.timeout = Duration::from_secs(120);
    Ok(r)
}

// ---------------- Tweak profile export / import ----------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakProfile {
    pub app: String,
    pub version: String,
    pub created: u64,
    pub computer: String,
    /// Tweak id -> "on" | "off" (custom states are not exported).
    pub tweaks: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileFile {
    pub file: String,
    pub created: u64,
    pub computer: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportChange {
    pub id: String,
    pub title: String,
    pub from: State,
    pub target: Target,
}

pub fn export_profile(tweaks: &Tweaks, dir: &Path, version: &str) -> AppResult<String> {
    std::fs::create_dir_all(dir)?;
    let mut map = BTreeMap::new();
    for t in tweaks.list() {
        match t.state {
            State::On => map.insert(t.id.to_string(), "on".to_string()),
            State::Off => map.insert(t.id.to_string(), "off".to_string()),
            State::Custom => None,
        };
    }
    let profile = TweakProfile {
        app: "Blob Optimizers".into(),
        version: version.into(),
        created: now_ms(),
        computer: std::env::var("COMPUTERNAME").unwrap_or_default(),
        tweaks: map,
    };
    let name = format!("blob-tweaks-{}.json", chrono_stamp());
    std::fs::write(dir.join(&name), serde_json::to_string_pretty(&profile)?)?;
    Ok(dir.join(name).to_string_lossy().into_owned())
}

/// yyyyMMdd-HHmmss in local time without pulling in a date crate.
fn chrono_stamp() -> String {
    crate::util::powershell::run("(Get-Date).ToString('yyyyMMdd-HHmmss')", Duration::from_secs(10))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| now_ms().to_string())
}

pub fn list_profiles(dir: &Path) -> Vec<ProfileFile> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<ProfileFile> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("json")))
        .filter_map(|e| {
            let p: TweakProfile = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            (p.app == "Blob Optimizers").then(|| ProfileFile { file: e.file_name().to_string_lossy().into_owned(), created: p.created, computer: p.computer, count: p.tweaks.len() })
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

/// What applying a saved profile would change on this PC.
pub fn preview_profile(tweaks: &Tweaks, dir: &Path, file: &str) -> AppResult<Vec<ImportChange>> {
    if file.contains(['\\', '/', ':']) {
        return Err(err("Invalid file name."));
    }
    let p: TweakProfile = serde_json::from_str(&std::fs::read_to_string(dir.join(file)).map_err(|_| err("Could not read that profile."))?)
        .map_err(|_| err("That file is not a Blob Optimizers tweak profile."))?;
    Ok(tweaks
        .list()
        .into_iter()
        .filter_map(|t| {
            let want = match p.tweaks.get(t.id)?.as_str() {
                "on" => State::On,
                "off" => State::Off,
                _ => return None,
            };
            (t.state != want).then(|| ImportChange {
                id: t.id.to_string(),
                title: t.title.to_string(),
                from: t.state,
                target: if want == State::On { Target::On } else { Target::Off },
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("blob-backup-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn profile_export_preview_round_trip() {
        let dir = temp("profile");
        let tweaks = Tweaks::new(dir.join("tw"));
        let path = export_profile(&tweaks, &dir, "test").unwrap();
        let file = std::path::Path::new(&path).file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(list_profiles(&dir).len(), 1);
        // Exported from this PC, so nothing differs.
        assert!(preview_profile(&tweaks, &dir, &file).unwrap().is_empty());

        // Flip one entry and the preview shows exactly that change.
        let mut p: TweakProfile = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let (id, v) = p.tweaks.iter().next().map(|(k, v)| (k.clone(), v.clone())).unwrap();
        p.tweaks.insert(id.clone(), if v == "on" { "off".into() } else { "on".into() });
        std::fs::write(&path, serde_json::to_string(&p).unwrap()).unwrap();
        let changes = preview_profile(&tweaks, &dir, &file).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].id, id);
        assert!(preview_profile(&tweaks, &dir, r"..\x.json").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn registry_backup_files_are_parsed_and_validated() {
        let dir = temp("reg");
        let text = "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\Software\\Foo]\r\n\"A\"=\"1\"\r\n\r\n[HKEY_CURRENT_USER\\Software\\Foo\\Sub]\r\n";
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(text.encode_utf16().flat_map(|u| u.to_le_bytes()));
        std::fs::write(dir.join("1-0.reg"), bytes).unwrap();
        let list = registry_backups(&dir);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].keys, vec![r"HKEY_CURRENT_USER\Software\Foo".to_string()]);
        assert!(!list[0].machine_wide);
        let job = import_registry_backup(&dir, "1-0.reg").unwrap();
        assert!(!job.requires_admin);
        for bad in [r"..\evil.reg", "C:evil.reg", "x.txt", "missing.reg"] {
            assert!(import_registry_backup(&dir, bad).is_err(), "{bad}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn protection_status_reads() {
        let s = protection_status();
        println!("{s:?}");
    }
}
