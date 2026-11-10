//! Leftover detection after an uninstall, and safe cleanup.
//!
//! Safety rules: only exact-name matches in per-app locations are reported (see the scan script);
//! every path is re-validated here against an allow-list of base folders and registry roots; folders
//! shared with another installed program are dropped; folders go to the Recycle Bin and registry keys
//! are exported to a .reg backup before deletion. The UI can only refer to items by scan id.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::inventory::InstalledApp;
use crate::error::AppResult;
use crate::util::powershell;

const SCAN: &str = include_str!("../../scripts/leftovers-scan.ps1");

/// Folder names that are never offered, wherever they appear (shared or system folders).
const PROTECTED_LEAVES: &[&str] = &[
    "windows", "microsoft", "common files", "microsoft shared", "packages", "temp", "tmp", "windowsapps",
    "programs", "microsoft.net", "windows defender", "windows nt", "internet explorer", "modifiablewindowsapps",
    "system32", "syswow64", "users", "public", "default", "appdata", "local", "roaming", "programdata",
    "program files", "program files (x86)", "package cache", "nvidia", "nvidia corporation", "amd", "intel",
    "google", "mozilla", "apple", "classes", "policies", "wow6432node", "clients", "registeredapplications",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeftoverKind {
    Folder,
    Registry,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLeftover {
    kind: LeftoverKind,
    path: String,
    size_bytes: u64,
    items: u32,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub id: String,
    pub kind: LeftoverKind,
    pub path: String,
    pub size_bytes: u64,
    pub items: u32,
    pub reason: String,
    /// Contains personal settings/data rather than program files.
    pub user_data: bool,
    pub requires_admin: bool,
    pub default_selected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverScan {
    pub scan_id: String,
    pub app_name: String,
    pub items: Vec<Leftover>,
}

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute())
}

fn norm(p: &Path) -> String {
    p.to_string_lossy().trim_end_matches('\\').to_lowercase()
}

fn is_strictly_inside(child: &str, parent: &str) -> bool {
    child.len() > parent.len() + 1 && child.starts_with(parent) && child.as_bytes()[parent.len()] == b'\\'
}

struct Bases {
    machine: Vec<String>,
    user: Vec<String>,
}

fn bases() -> Bases {
    let machine = ["ProgramFiles", "ProgramFiles(x86)", "ProgramData"].iter().filter_map(|v| env_dir(v)).map(|p| norm(&p)).collect();
    let user = ["APPDATA", "LOCALAPPDATA"].iter().filter_map(|v| env_dir(v)).map(|p| norm(&p)).collect();
    Bases { machine, user }
}

fn leaf_ok(path: &str) -> bool {
    let leaf = path.rsplit('\\').next().unwrap_or("").trim().to_lowercase();
    leaf.len() >= 3 && !PROTECTED_LEAVES.contains(&leaf.as_str())
}

/// Validates a folder; returns (requires_admin, user_data) when it may be offered.
fn check_folder(path: &str, install_location: Option<&str>, b: &Bases) -> Option<(bool, bool)> {
    if !leaf_ok(path) || path.contains("..") {
        return None;
    }
    let p = path.trim_end_matches('\\').to_lowercase();
    if b.machine.iter().any(|base| is_strictly_inside(&p, base)) {
        // ProgramData holds app data/settings shared by all users; Program Files holds program files.
        let data = b.machine.iter().any(|base| base.ends_with("programdata") && is_strictly_inside(&p, base));
        return Some((true, data));
    }
    if b.user.iter().any(|base| is_strictly_inside(&p, base)) {
        // Local\Programs holds program files; the rest of AppData holds the user's settings.
        let programs = b.user.iter().any(|base| is_strictly_inside(&p, &format!("{base}\\programs")));
        return Some((false, !programs));
    }
    // The program's own install folder may live elsewhere (e.g. C:\Games\X) but never at a drive root,
    // in Windows, or directly in a profile.
    let own = install_location.map(|l| l.trim_end_matches('\\').to_lowercase()).as_deref() == Some(p.as_str());
    let depth = p.split('\\').filter(|s| !s.is_empty()).count();
    let windir = env_dir("SystemRoot").map(|w| norm(&w)).unwrap_or_else(|| r"c:\windows".into());
    let profile = env_dir("USERPROFILE").map(|w| norm(&w)).unwrap_or_default();
    if own && depth >= 3 && !p.starts_with(&windir) && !(is_strictly_inside(&p, &profile) && depth <= 4) {
        return Some((true, false));
    }
    None
}

fn check_registry(path: &str) -> Option<bool> {
    const ROOTS: &[(&str, bool)] = &[
        (r"hkcu:\software\", false),
        (r"hklm:\software\wow6432node\", true),
        (r"hklm:\software\", true),
    ];
    let lower = path.to_lowercase();
    let (root, admin) = ROOTS.iter().find(|(r, _)| lower.starts_with(r))?;
    let rest = &lower[root.len()..];
    if rest.is_empty() || rest.contains("..") || !leaf_ok(path) {
        return None;
    }
    // Never a top-level vendor key such as Microsoft or Classes.
    let first = rest.split('\\').next().unwrap_or("");
    if PROTECTED_LEAVES.contains(&first) {
        return None;
    }
    Some(*admin)
}

fn overlaps(path: &str, others: &[&InstalledApp]) -> bool {
    let p = path.trim_end_matches('\\').to_lowercase();
    others.iter().filter_map(|a| a.install_location.as_deref()).map(|l| l.trim_end_matches('\\').to_lowercase()).filter(|l| l.len() > 3).any(|l| {
        l == p || is_strictly_inside(&l, &p) || is_strictly_inside(&p, &l)
    })
}

fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

/// Scans for leftovers of `app` (a snapshot taken before uninstalling). `installed` is the current
/// inventory and is used to drop anything shared with programs that are still installed.
pub fn scan(app: &InstalledApp, installed: &[InstalledApp]) -> AppResult<Vec<Leftover>> {
    let script = SCAN
        .replace("__NAME__", &ps_quote(&app.name))
        .replace("__PUBLISHER__", &ps_quote(&app.publisher))
        .replace("__LOCATION__", &ps_quote(app.install_location.as_deref().unwrap_or("")));
    let raw: Vec<RawLeftover> = powershell::run_json(&script, Duration::from_secs(120))?;
    let others: Vec<&InstalledApp> = installed.iter().filter(|a| a.id != app.id).collect();
    let b = bases();

    let mut out = Vec::new();
    for (i, r) in raw.into_iter().enumerate() {
        let checked = match r.kind {
            LeftoverKind::Folder => {
                if overlaps(&r.path, &others) {
                    continue;
                }
                check_folder(&r.path, app.install_location.as_deref(), &b)
            }
            LeftoverKind::Registry => check_registry(&r.path).map(|admin| (admin, true)),
        };
        let Some((requires_admin, user_data)) = checked else { continue };
        out.push(Leftover {
            id: i.to_string(),
            kind: r.kind,
            default_selected: r.kind == LeftoverKind::Folder && !user_data,
            path: r.path,
            size_bytes: r.size_bytes,
            items: r.items,
            reason: r.reason,
            user_data,
            requires_admin,
        });
    }
    Ok(out)
}

/// Re-validates the chosen items and builds the cleanup script.
pub fn cleanup_script(items: &[Leftover], backup_dir: &Path) -> String {
    let mut s = format!("{}$failed = 0\n", crate::util::recycle::PS_RECYCLE_BIN);
    s.push_str(&format!("$backup = '{}'\nNew-Item -ItemType Directory -Force -Path $backup | Out-Null\n", ps_quote(&backup_dir.to_string_lossy())));
    let total = items.len().max(1);
    for (i, item) in items.iter().enumerate() {
        let path = ps_quote(&item.path);
        s.push_str(&format!("'##PROGRESS {} Cleaning up'\n", i * 100 / total));
        match item.kind {
            LeftoverKind::Folder => s.push_str(&format!(
                r#"if (Test-Path -LiteralPath '{path}') {{
    $r = [BlobRecycle]::ToRecycleBin('{path}')
    if ($r -eq 0 -and -not (Test-Path -LiteralPath '{path}')) {{ "Moved to Recycle Bin: {path}" }}
    else {{ $failed++; [Console]::Error.WriteLine("Could not move to Recycle Bin (code $r, files may be in use): {path}") }}
}}
"#
            )),
            LeftoverKind::Registry => {
                let reg_path = item.path.replacen(":\\", "\\", 1);
                let file = format!("{}-{}.reg", crate::engine::types::now_ms(), item.id);
                s.push_str(&format!(
                    r#"if (Test-Path -LiteralPath '{path}') {{
    $file = Join-Path $backup '{file}'
    & reg.exe export '{reg}' $file /y | Out-Null
    if ($LASTEXITCODE -eq 0) {{
        Remove-Item -LiteralPath '{path}' -Recurse -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath '{path}') {{ $failed++; [Console]::Error.WriteLine("Could not delete registry key: {path}") }}
        else {{ "Deleted registry key {path} (backup: $file)" }}
    }} else {{ $failed++; [Console]::Error.WriteLine("Backup failed, key left untouched: {path}") }}
}}
"#,
                    reg = ps_quote(&reg_path)
                ));
            }
        }
    }
    s.push_str("$global:LASTEXITCODE = 0\n'##PROGRESS 100 Done'\nif ($failed) { exit 1 }\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_rules() {
        let b = Bases {
            machine: vec![r"c:\program files".into(), r"c:\program files (x86)".into(), r"c:\programdata".into()],
            user: vec![r"c:\users\me\appdata\roaming".into(), r"c:\users\me\appdata\local".into()],
        };
        assert_eq!(check_folder(r"C:\Program Files\Foo App", None, &b), Some((true, false)));
        assert_eq!(check_folder(r"C:\ProgramData\Foo App", None, &b), Some((true, true)));
        assert_eq!(check_folder(r"C:\Users\me\AppData\Roaming\FooApp", None, &b), Some((false, true)));
        assert_eq!(check_folder(r"C:\Users\me\AppData\Local\Programs\FooApp", None, &b), Some((false, false)));
        // Never the bases themselves, shared folders or random locations.
        assert_eq!(check_folder(r"C:\Program Files", None, &b), None);
        assert_eq!(check_folder(r"C:\Program Files\Common Files", None, &b), None);
        assert_eq!(check_folder(r"C:\Users\me\AppData\Local\Microsoft", None, &b), None);
        assert_eq!(check_folder(r"C:\Games\Foo", None, &b), None);
        assert_eq!(check_folder(r"C:\Program Files\..\Windows", None, &b), None);
        // ...except the app's own install folder, unless it is too shallow.
        assert_eq!(check_folder(r"C:\Riot Games\VALORANT\live", Some(r"C:\Riot Games\VALORANT\live\"), &b), Some((true, false)));
        assert_eq!(check_folder(r"C:\Games", Some(r"C:\Games"), &b), None);
    }

    #[test]
    fn registry_rules() {
        assert_eq!(check_registry(r"HKCU:\Software\Foo App"), Some(false));
        assert_eq!(check_registry(r"HKLM:\SOFTWARE\Vendor\Foo App"), Some(true));
        assert_eq!(check_registry(r"HKCU:\Software\Microsoft"), None);
        assert_eq!(check_registry(r"HKCU:\Software\Microsoft\Foo"), None);
        assert_eq!(check_registry(r"HKLM:\SYSTEM\CurrentControlSet\Services\Foo"), None);
        assert_eq!(check_registry(r"HKCU:\Software"), None);
    }

    #[test]
    fn cleanup_script_quotes_paths() {
        let item = Leftover {
            id: "0".into(),
            kind: LeftoverKind::Folder,
            path: r"C:\Program Files\O'Brien App".into(),
            size_bytes: 0,
            items: 0,
            reason: String::new(),
            user_data: false,
            requires_admin: true,
            default_selected: true,
        };
        let s = cleanup_script(&[item], Path::new(r"C:\backup"));
        assert!(s.contains("O''Brien App"));
        assert!(!s.contains("Remove-Item -LiteralPath 'C:\\Program Files"));
    }
}
