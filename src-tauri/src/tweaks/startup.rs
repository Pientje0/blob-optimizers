//! Startup manager: list, enable/disable (Task Manager's StartupApproved mechanism, fully reversible),
//! remove with backup, restore removed entries.

use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::engine::types::{now_ms, Callback, JobRequest, JobStatus, Shell};
use crate::error::{AppError, AppResult};
use crate::util::powershell;
use crate::util::registry::{self, Hive, RegData};
use crate::util::serde_helpers::one_or_many;

const LIST: &str = include_str!("../../scripts/startup-list.ps1");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEntry {
    id: String,
    source: String,
    location: String,
    name: String,
    command: String,
    path: Option<String>,
    publisher: Option<String>,
    enabled: bool,
    scope: String,
    impact_ms: Option<u32>,
    #[serde(default)]
    policy: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawList {
    impact_available: bool,
    #[serde(deserialize_with = "one_or_many")]
    entries: Vec<RawEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Advice {
    /// Starts with Windows only for convenience.
    SafeToDisable,
    /// Hardware, sync or security helpers: disabling has a visible effect.
    Caution,
    /// Windows Security: may be disabled (tray icon only) but never removed.
    Keep,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupEntry {
    pub id: String,
    pub source: String,
    pub location: String,
    pub name: String,
    pub command: String,
    pub path: Option<String>,
    pub publisher: Option<String>,
    pub enabled: bool,
    pub scope: String,
    pub impact_ms: Option<u32>,
    pub advice: Advice,
    pub note: &'static str,
    pub removable: bool,
    /// Set by a group policy; cannot be changed here.
    pub policy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedEntry {
    pub name: String,
    pub location: String,
    pub hive: Hive,
    pub key: String,
    pub data: RegData,
    pub removed_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupList {
    pub impact_available: bool,
    pub entries: Vec<StartupEntry>,
    pub removed: Vec<RemovedEntry>,
}

struct Rule {
    re: Regex,
    advice: Advice,
    note: &'static str,
}

static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    let r = |p: &str, advice, note| Rule { re: Regex::new(&format!("(?i){p}")).unwrap(), advice, note };
    vec![
        r(r"SecurityHealth|Windows Security", Advice::Keep, "Windows Security tray icon. Protection keeps running if disabled, but you won't see its alerts in the tray."),
        r(r"McAfee|Norton|Avast|\bAVG\b|Kaspersky|Bitdefender|ESET|Malwarebytes|Sophos", Advice::Caution, "Security software. Disabling it can leave real-time protection or alerts off until you open it."),
        // Short vendor names need word boundaries: "AMD" also occurs inside "ProgramData".
        r(r"RtkAud|Realtek|Nahimic|\bWaves\b|MaxxAudio|Dolby|\bDTS\b|SoundBlaster|Sonic Studio", Advice::Caution, "Audio driver helper. Sound enhancements, jack detection or the audio console may stop working."),
        r(r"\bAMD|Radeon|\bcncmd|NVIDIA|GeForce|Intel.*Graphics|\bigfx", Advice::Caution, "Graphics driver helper. GPU settings, overlays or hotkeys may not apply until you open the app."),
        r(r"\bLogi(?:tech|Options|\b)|LGHUB|Razer|Synapse|Corsair|iCUE|SteelSeries|HyperX|Wooting|ReMouse", Advice::Caution, "Device software. Mouse/keyboard profiles, macros, DPI or lighting apply only while it runs."),
        r(r"OneDrive|Dropbox|Google ?Drive|iCloud|MEGAsync|Nextcloud", Advice::Caution, "Cloud sync. Files stop syncing until you open the app."),
        r(r"Bluetooth|TP-?Link|\bWi-?Fi\b|Synaptics|\bElan\b|Precision Touchpad", Advice::Caution, "Hardware utility. Related device features may stop working."),
        r(r"Steam|Epic ?Games|EpicGamesLauncher|Riot|Battle\.net|EA ?app|Ubisoft|GOG|Discord|Spotify|Teams|Skype|Zoom|Slack|WhatsApp|Telegram|Opera|Brave|Edge|Chrome|Ollama|Voicemod|CurseForge|Overwolf", Advice::SafeToDisable, "Starts with Windows only for convenience. Disabling is safe; open it yourself when you need it."),
    ]
});

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, path: &str, publisher: &str) -> RawEntry {
        RawEntry {
            id: String::new(),
            source: "registry".into(),
            location: String::new(),
            name: name.into(),
            command: String::new(),
            path: Some(path.into()),
            publisher: Some(publisher.into()),
            enabled: true,
            scope: "user".into(),
            impact_ms: None,
            policy: false,
        }
    }

    #[test]
    fn classification_uses_whole_words() {
        let discord = entry("Discord", r"C:\ProgramData\SquirrelMachineInstalls\Discord.exe", "Discord Inc.");
        assert_eq!(classify(&discord).0, Advice::SafeToDisable);
        assert_eq!(classify(&entry("AMDNoiseSuppression", r"C:\Program Files\AMD\x.exe", "Advanced Micro Devices")).0, Advice::Caution);
        assert_eq!(classify(&entry("SecurityHealth", r"C:\Windows\system32\SecurityHealthSystray.exe", "Microsoft")).0, Advice::Keep);
        assert_eq!(classify(&entry("LGHUB", r"C:\Program Files\LGHUB\lghub.exe", "Logitech")).0, Advice::Caution);
    }
}

fn classify(e: &RawEntry) -> (Advice, &'static str) {
    let hay = format!("{} {} {}", e.name, e.path.as_deref().unwrap_or(""), e.publisher.as_deref().unwrap_or(""));
    RULES
        .iter()
        .find(|r| r.re.is_match(&hay))
        .map(|r| (r.advice, r.note))
        .unwrap_or((Advice::SafeToDisable, "Disabling only stops it from starting automatically; the app keeps working when you open it."))
}

/// (hive, Run key, StartupApproved subkey) for a registry/folder location.
fn location(loc: &str) -> Option<(Hive, &'static str, &'static str)> {
    Some(match loc {
        r"HKCU\Run" => (Hive::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Run", "Run"),
        r"HKLM\Run" => (Hive::LocalMachine, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", "Run"),
        r"HKLM\Run32" => (Hive::LocalMachine, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run", "Run32"),
        "UserStartup" => (Hive::CurrentUser, "", "StartupFolder"),
        "CommonStartup" => (Hive::LocalMachine, "", "StartupFolder"),
        _ => return None,
    })
}

fn q(s: &str) -> String {
    s.replace('\'', "''")
}

fn err(m: impl Into<String>) -> AppError {
    AppError::Other(m.into())
}

pub struct Startup {
    path: PathBuf,
    removed: Arc<Mutex<Vec<RemovedEntry>>>,
}

impl Startup {
    pub fn new(data_dir: PathBuf) -> Self {
        let path = data_dir.join("startup-removed.json");
        let removed = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Self { path, removed: Arc::new(Mutex::new(removed)) }
    }

    fn save(path: &PathBuf, items: &[RemovedEntry]) {
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(json) = serde_json::to_string_pretty(items) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn list(&self) -> AppResult<StartupList> {
        let raw: RawList = powershell::run_json(LIST, Duration::from_secs(90))?;
        let entries = raw
            .entries
            .into_iter()
            .map(|e| {
                let (advice, note) = classify(&e);
                StartupEntry {
                    removable: e.source != "store" && advice != Advice::Keep,
                    advice,
                    note,
                    id: e.id,
                    source: e.source,
                    location: e.location,
                    name: e.name,
                    command: e.command,
                    path: e.path,
                    publisher: e.publisher,
                    enabled: e.enabled,
                    scope: e.scope,
                    impact_ms: e.impact_ms,
                    policy: e.policy,
                }
            })
            .collect();
        Ok(StartupList { impact_available: raw.impact_available, entries, removed: self.removed.lock().map(|r| r.clone()).unwrap_or_default() })
    }

    fn find(&self, id: &str) -> AppResult<StartupEntry> {
        self.list()?.entries.into_iter().find(|e| e.id == id).ok_or_else(|| err("That startup entry no longer exists. Refresh the list."))
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> AppResult<JobRequest> {
        let e = self.find(id)?;
        if e.policy {
            return Err(err("This startup task is controlled by a group policy."));
        }
        let script = if e.source == "store" {
            let family = &e.location;
            let task = e.id.rsplit('|').next().unwrap_or("");
            format!(
                "Set-ItemProperty -LiteralPath 'HKCU:\\Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\CurrentVersion\\AppModel\\SystemAppData\\{}\\{}' -Name State -Value {} -Type DWord\n",
                q(family),
                q(task),
                if enabled { 2 } else { 1 }
            )
        } else {
            let (hive, _, approved) = location(&e.location).ok_or_else(|| err("Unknown startup location."))?;
            let bytes = if enabled { "[byte[]](2,0,0,0,0,0,0,0,0,0,0,0)" } else { "([byte[]](3,0,0,0) + [BitConverter]::GetBytes([DateTime]::Now.ToFileTime()))" };
            format!(
                "$k = '{}\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\{approved}'\nif (-not (Test-Path -LiteralPath $k)) {{ New-Item -Path $k -Force | Out-Null }}\nNew-ItemProperty -LiteralPath $k -Name '{}' -PropertyType Binary -Value {bytes} -Force | Out-Null\n",
                hive.ps(),
                q(&e.name)
            )
        };
        let verb = if enabled { "Enable" } else { "Disable" };
        let mut r = JobRequest::internal(format!("{verb} startup: {}", e.name), Shell::PowerShell, format!("{script}'{} will {}start with Windows.'", q(&e.name), if enabled { "" } else { "no longer " }));
        r.operation_id = Some("startup.toggle".into());
        r.requires_admin = e.scope == "machine";
        r.timeout = Duration::from_secs(60);
        Ok(r)
    }

    pub fn remove(&self, id: &str) -> AppResult<JobRequest> {
        let e = self.find(id)?;
        if !e.removable {
            return Err(err(format!("{} cannot be removed here; disable it instead.", e.name)));
        }
        let (hive, run_key, approved) = location(&e.location).ok_or_else(|| err("Unknown startup location."))?;
        let approved_del = format!(
            "Remove-ItemProperty -LiteralPath '{}\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\{approved}' -Name '{}' -ErrorAction SilentlyContinue\n",
            hive.ps(),
            q(&e.name)
        );
        let mut r;
        if e.source == "folder" {
            let file = e.command.clone();
            let script = format!(
                "{}$r = [BlobRecycle]::ToRecycleBin('{f}')\nif ($r -ne 0 -or (Test-Path -LiteralPath '{f}')) {{ throw 'Could not move the shortcut to the Recycle Bin.' }}\n{approved_del}'Moved {n} to the Recycle Bin. Restore it from there to undo.'",
                crate::util::recycle::PS_RECYCLE_BIN,
                f = q(&file),
                n = q(&e.name)
            );
            r = JobRequest::internal(format!("Remove startup: {}", e.name), Shell::PowerShell, script);
        } else {
            // Keep the exact value (including its type) so it can be restored.
            let data = registry::read(hive, run_key, &e.name).ok_or_else(|| err("The startup value could not be read."))?;
            let backup = RemovedEntry { name: e.name.clone(), location: e.location.clone(), hive, key: run_key.into(), data, removed_at: now_ms() };
            let script = format!("{}{approved_del}'Removed {}. It can be restored from the Startup page.'", registry::ps_delete_value(hive, run_key, &e.name), q(&e.name));
            r = JobRequest::internal(format!("Remove startup: {}", e.name), Shell::PowerShell, script);
            let store = self.removed.clone();
            let path = self.path.clone();
            r.on_complete = Some(Callback(Arc::new(move |entry| {
                if entry.status == JobStatus::Succeeded {
                    if let Ok(mut v) = store.lock() {
                        v.push(backup.clone());
                        Self::save(&path, &v);
                    }
                }
            })));
        }
        r.operation_id = Some("startup.remove".into());
        r.requires_admin = e.scope == "machine";
        r.timeout = Duration::from_secs(60);
        Ok(r)
    }

    pub fn restore(&self, name: &str, location: &str) -> AppResult<JobRequest> {
        let item = self
            .removed
            .lock()
            .ok()
            .and_then(|v| v.iter().find(|r| r.name == name && r.location == location).cloned())
            .ok_or_else(|| err("Nothing to restore."))?;
        let script = format!("{}'Restored {}.'", registry::ps_set(item.hive, &item.key, &item.name, &item.data), q(&item.name));
        let mut r = JobRequest::internal(format!("Restore startup: {}", item.name), Shell::PowerShell, script);
        r.operation_id = Some("startup.restore".into());
        r.requires_admin = item.hive == Hive::LocalMachine;
        r.timeout = Duration::from_secs(60);
        let store = self.removed.clone();
        let path = self.path.clone();
        let (n, l) = (item.name.clone(), item.location.clone());
        r.on_complete = Some(Callback(Arc::new(move |entry| {
            if entry.status == JobStatus::Succeeded {
                if let Ok(mut v) = store.lock() {
                    v.retain(|x| !(x.name == n && x.location == l));
                    Self::save(&path, &v);
                }
            }
        })));
        Ok(r)
    }
}
