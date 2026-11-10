//! Blob Optimizers preferences, stored in `settings.json` in the app data folder.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::types::{JobRequest, Shell};
use crate::error::{AppError, AppResult};
use crate::util::registry::{self, Hive};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Blob Optimizers";
/// Where official releases are published.
pub const DEFAULT_UPDATE_REPO: &str = "Pientje0/blob-optimizers";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "dark" | "light" | "system"
    pub theme: String,
    /// Accent preset id.
    pub accent: String,
    /// Live monitoring interval in milliseconds.
    pub refresh_ms: u32,
    /// Show a toast when an operation succeeds (failures are always shown).
    pub notify_success: bool,
    pub start_minimized: bool,
    /// Pre-tick "create a restore point" for batch changes.
    pub restore_point_default: bool,
    /// Operation history entries to keep.
    pub history_limit: usize,
    /// Preset used by the Dashboard's quick action.
    pub default_preset: String,
    pub language: String,
    /// Check GitHub for a newer release when the app starts.
    pub check_updates: bool,
    /// GitHub repository (`owner/name`) that publishes Blob Optimizers releases; empty = not configured.
    pub update_repo: String,
    /// Read from the registry, not stored.
    #[serde(skip_deserializing)]
    pub start_with_windows: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            accent: "violet".into(),
            refresh_ms: 1500,
            notify_success: true,
            start_minimized: false,
            restore_point_default: true,
            history_limit: 500,
            default_preset: "gaming".into(),
            language: "en".into(),
            check_updates: true,
            update_repo: DEFAULT_UPDATE_REPO.into(),
            start_with_windows: false,
        }
    }
}

impl Settings {
    fn validate(&self) -> AppResult<()> {
        let bad = |m: &str| Err(AppError::Other(m.into()));
        if !["dark", "light", "system"].contains(&self.theme.as_str()) {
            return bad("Unknown theme.");
        }
        if !(500..=10_000).contains(&self.refresh_ms) {
            return bad("Refresh interval must be between 0.5 and 10 seconds.");
        }
        if !(50..=5000).contains(&self.history_limit) {
            return bad("History size must be between 50 and 5000.");
        }
        if !["balanced", "gaming", "maximum"].contains(&self.default_preset.as_str()) {
            return bad("Unknown preset.");
        }
        if !self.update_repo.is_empty() && !crate::updates::valid_repo(&self.update_repo) {
            return bad("The release repository must look like owner/name.");
        }
        if self.accent.len() > 20 || !self.accent.chars().all(|c| c.is_ascii_alphanumeric()) {
            return bad("Unknown accent colour.");
        }
        Ok(())
    }
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

fn run_command() -> AppResult<String> {
    let exe = std::env::current_exe()?;
    Ok(format!("\"{}\" --minimized-if-set", exe.display()))
}

impl SettingsStore {
    pub fn load(data_dir: PathBuf) -> Self {
        let path = data_dir.join("settings.json");
        let current = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Self { path, current: Mutex::new(current) }
    }

    pub fn get(&self) -> Settings {
        let mut s = self.current.lock().map(|s| s.clone()).unwrap_or_default();
        s.start_with_windows = registry::read(Hive::CurrentUser, RUN_KEY, RUN_VALUE).is_some();
        s
    }

    pub fn save(&self, s: &Settings) -> AppResult<()> {
        s.validate()?;
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&self.path, serde_json::to_string_pretty(s)?)?;
        if let Ok(mut c) = self.current.lock() {
            *c = s.clone();
        }
        Ok(())
    }

    /// Adds or removes Blob Optimizers from the current user's startup programs.
    pub fn start_with_windows(&self, on: bool) -> AppResult<JobRequest> {
        let script = if on {
            let cmd = run_command()?;
            format!("{}'Blob Optimizers will start when you sign in.'", registry::ps_set(Hive::CurrentUser, RUN_KEY, RUN_VALUE, &registry::RegData::String(cmd)))
        } else {
            format!("{}'Blob Optimizers will no longer start when you sign in.'", registry::ps_delete_value(Hive::CurrentUser, RUN_KEY, RUN_VALUE))
        };
        let mut r = JobRequest::internal(if on { "Start Blob Optimizers with Windows" } else { "Don't start Blob Optimizers with Windows" }, Shell::PowerShell, script);
        r.operation_id = Some("settings.autostart".into());
        r.timeout = Duration::from_secs(60);
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_and_validate() {
        let dir = std::env::temp_dir().join(format!("blob-settings-test-{}", std::process::id()));
        let store = SettingsStore::load(dir.clone());
        assert_eq!(store.get().theme, "dark");
        let mut s = store.get();
        s.theme = "light".into();
        s.refresh_ms = 2000;
        store.save(&s).unwrap();
        assert_eq!(SettingsStore::load(dir.clone()).get().theme, "light");
        for bad in [
            Settings { theme: "neon".into(), ..s.clone() },
            Settings { refresh_ms: 50, ..s.clone() },
            Settings { history_limit: 1, ..s.clone() },
            Settings { accent: "x;y".into(), ..s.clone() },
            Settings { update_repo: "a/b;calc".into(), ..s.clone() },
        ] {
            assert!(store.save(&bad).is_err());
        }
        let job = store.start_with_windows(true).unwrap();
        assert!(job.command.contains("--minimized-if-set") && !job.requires_admin);
        let _ = std::fs::remove_dir_all(dir);
    }
}
