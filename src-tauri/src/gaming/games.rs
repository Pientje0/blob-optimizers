//! Installed game detection and per-game profiles (GPU preference, fullscreen optimizations, CPU priority).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::types::{JobRequest, Shell};
use crate::error::{AppError, AppResult};
use crate::util::powershell;
use crate::util::registry::{self, Hive, RegData};
use crate::util::serde_helpers::one_or_many;

const DETECT: &str = include_str!("../../scripts/games-detect.ps1");
const GPU_PREFS: &str = r"Software\Microsoft\DirectX\UserGpuPreferences";
const LAYERS: &str = r"Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers";
const IFEO: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options";
const FSO_FLAG: &str = "DISABLEDXMAXIMIZEDWINDOWEDMODE";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub source: String,
    pub name: String,
    pub install_dir: String,
    /// Executable the profile applies to (known launch exe, or chosen by the user).
    pub exe: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GpuPreference {
    Default,
    PowerSaving,
    HighPerformance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Priority {
    Normal,
    AboveNormal,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub gpu: GpuPreference,
    /// Windows fullscreen optimizations (on is the Windows default).
    pub fullscreen_optimizations: bool,
    pub priority: Priority,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExeCandidate {
    pub path: String,
    pub size_bytes: u64,
}

/// User choices that detection can't provide: manually added games and chosen executables.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    custom: Vec<Game>,
    chosen_exe: std::collections::HashMap<String, String>,
}

pub struct Games {
    path: PathBuf,
    saved: Mutex<Saved>,
}

fn err(m: impl Into<String>) -> AppError {
    AppError::Other(m.into())
}

/// Accepts only absolute paths to existing .exe files without characters that would need escaping.
fn validate_exe(path: &str) -> AppResult<String> {
    let p = Path::new(path);
    let ok = p.is_absolute()
        && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        // Paths are embedded in single-quoted PowerShell strings with quotes doubled, so only
        // characters that can't occur in a valid path are refused.
        && !path.chars().any(|c| c == '"' || c.is_control())
        && p.is_file();
    if !ok {
        return Err(err("Choose an existing .exe file (full path)."));
    }
    Ok(path.to_string())
}

pub fn read_profile(exe: &str) -> Profile {
    let gpu = match registry::read(Hive::CurrentUser, GPU_PREFS, exe) {
        Some(RegData::String(s)) if s.contains("GpuPreference=2") => GpuPreference::HighPerformance,
        Some(RegData::String(s)) if s.contains("GpuPreference=1") => GpuPreference::PowerSaving,
        _ => GpuPreference::Default,
    };
    let fso_disabled = matches!(registry::read(Hive::CurrentUser, LAYERS, exe), Some(RegData::String(s)) if s.to_uppercase().contains(FSO_FLAG));
    let file = Path::new(exe).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let priority = match registry::read(Hive::LocalMachine, &format!(r"{IFEO}\{file}\PerfOptions"), "CpuPriorityClass") {
        Some(RegData::Dword(3)) => Priority::High,
        Some(RegData::Dword(6)) => Priority::AboveNormal,
        _ => Priority::Normal,
    };
    Profile { gpu, fullscreen_optimizations: !fso_disabled, priority }
}

impl Games {
    pub fn new(data_dir: PathBuf) -> Self {
        let path = data_dir.join("games.json");
        let saved = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Self { path, saved: Mutex::new(saved) }
    }

    fn persist(&self, s: &Saved) {
        if let Some(d) = self.path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(json) = serde_json::to_string_pretty(s) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    pub fn list(&self) -> AppResult<Vec<Game>> {
        #[derive(Deserialize)]
        struct W(#[serde(deserialize_with = "one_or_many")] Vec<Game>);
        let W(mut games) = powershell::run_json(DETECT, Duration::from_secs(60))?;
        let saved = self.saved.lock().map(|s| s.clone()).unwrap_or_default();
        games.extend(saved.custom.iter().filter(|g| g.exe.as_deref().is_some_and(|e| Path::new(e).is_file())).cloned());
        for g in &mut games {
            if let Some(exe) = saved.chosen_exe.get(&g.id).filter(|e| Path::new(e).is_file()) {
                g.exe = Some(exe.clone());
            }
        }
        games.sort_by_key(|a| a.name.to_lowercase());
        games.dedup_by(|a, b| a.id == b.id);
        Ok(games)
    }

    pub fn add_custom(&self, name: &str, exe: &str) -> AppResult<Game> {
        let exe = validate_exe(exe)?;
        let name = name.trim();
        if name.is_empty() || name.len() > 100 {
            return Err(err("Enter a name of up to 100 characters."));
        }
        let dir = Path::new(&exe).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let game = Game { id: format!("custom|{}", exe.to_lowercase()), source: "custom".into(), name: name.into(), install_dir: dir, exe: Some(exe) };
        if let Ok(mut s) = self.saved.lock() {
            s.custom.retain(|g| g.id != game.id);
            s.custom.push(game.clone());
            let snapshot = s.clone();
            drop(s);
            self.persist(&snapshot);
        }
        Ok(game)
    }

    pub fn remove_custom(&self, id: &str) {
        if let Ok(mut s) = self.saved.lock() {
            s.custom.retain(|g| g.id != id);
            let snapshot = s.clone();
            drop(s);
            self.persist(&snapshot);
        }
    }

    pub fn choose_exe(&self, game_id: &str, exe: &str) -> AppResult<()> {
        let exe = validate_exe(exe)?;
        let game = self.list()?.into_iter().find(|g| g.id == game_id).ok_or_else(|| err("Unknown game."))?;
        if !exe.to_lowercase().starts_with(&game.install_dir.to_lowercase()) {
            return Err(err("Choose an executable inside the game's folder."));
        }
        if let Ok(mut s) = self.saved.lock() {
            s.chosen_exe.insert(game_id.into(), exe);
            let snapshot = s.clone();
            drop(s);
            self.persist(&snapshot);
        }
        Ok(())
    }

    /// Likely game executables in the install folder, best guesses first.
    pub fn candidates(&self, game_id: &str) -> AppResult<Vec<ExeCandidate>> {
        let game = self.list()?.into_iter().find(|g| g.id == game_id).ok_or_else(|| err("Unknown game."))?;
        let skip = regex::Regex::new(r"(?i)unins|setup|install|redist|vcredist|dxsetup|dotnet|crash|report|prereq|easyanticheat|battleye|cefprocess|helper|update|unitycrash|ue4|bootstrap").unwrap();
        let mut out = Vec::new();
        let mut stack = vec![(PathBuf::from(&game.install_dir), 0)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() && depth < 4 {
                    stack.push((p, depth + 1));
                } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe")) {
                    let name = p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    if skip.is_match(&name) {
                        continue;
                    }
                    let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                    out.push(ExeCandidate { path: p.to_string_lossy().into_owned(), size_bytes: size });
                }
            }
        }
        // Unreal "-Shipping" builds are the real game; otherwise the largest executable usually is.
        out.sort_by(|a, b| (b.path.contains("Shipping"), b.size_bytes).cmp(&(a.path.contains("Shipping"), a.size_bytes)));
        out.truncate(15);
        Ok(out)
    }

    pub fn set_profile(&self, exe: &str, p: Profile) -> AppResult<JobRequest> {
        let exe = validate_exe(exe)?;
        let current = read_profile(&exe);
        let file = Path::new(&exe).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let mut s = String::new();
        if p.gpu != current.gpu {
            s.push_str(&match p.gpu {
                GpuPreference::Default => registry::ps_delete_value(Hive::CurrentUser, GPU_PREFS, &exe),
                GpuPreference::PowerSaving => registry::ps_set(Hive::CurrentUser, GPU_PREFS, &exe, &RegData::String("GpuPreference=1;".into())),
                GpuPreference::HighPerformance => registry::ps_set(Hive::CurrentUser, GPU_PREFS, &exe, &RegData::String("GpuPreference=2;".into())),
            });
        }
        if p.fullscreen_optimizations != current.fullscreen_optimizations {
            // Keep any other compatibility flags already set for this exe (e.g. RUNASADMIN).
            let existing = match registry::read(Hive::CurrentUser, LAYERS, &exe) {
                Some(RegData::String(v)) => v,
                _ => String::new(),
            };
            let mut flags: Vec<String> = existing.split_whitespace().filter(|f| *f != "~" && !f.eq_ignore_ascii_case(FSO_FLAG)).map(String::from).collect();
            if !p.fullscreen_optimizations {
                flags.push(FSO_FLAG.into());
            }
            s.push_str(&if flags.is_empty() {
                registry::ps_delete_value(Hive::CurrentUser, LAYERS, &exe)
            } else {
                registry::ps_set(Hive::CurrentUser, LAYERS, &exe, &RegData::String(format!("~ {}", flags.join(" "))))
            });
        }
        let admin = p.priority != current.priority;
        if admin {
            let key = format!(r"{IFEO}\{file}\PerfOptions");
            s.push_str(&match p.priority {
                Priority::Normal => registry::ps_delete_value(Hive::LocalMachine, &key, "CpuPriorityClass"),
                Priority::AboveNormal => registry::ps_set(Hive::LocalMachine, &key, "CpuPriorityClass", &RegData::Dword(6)),
                Priority::High => registry::ps_set(Hive::LocalMachine, &key, "CpuPriorityClass", &RegData::Dword(3)),
            });
        }
        if s.is_empty() {
            return Err(err("Nothing changed."));
        }
        s.push_str(&format!("'Profile saved for {}. Restart the game to apply it.'", file.replace('\'', "''")));
        let mut r = JobRequest::internal(format!("Game profile: {file}"), Shell::PowerShell, s);
        r.operation_id = Some("gaming.profile".into());
        r.requires_admin = admin;
        r.timeout = Duration::from_secs(60);
        Ok(r)
    }
}
