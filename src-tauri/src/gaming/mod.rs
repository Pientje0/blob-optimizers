//! Gaming: presets (built from catalogue tweaks plus a power plan), game profiles, displays, FPS.

pub mod display;
pub mod fps;
pub mod games;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::engine::types::{now_ms, Callback, JobRequest, JobStatus};
use crate::error::{AppError, AppResult};
use crate::tweaks::model::State;
use crate::tweaks::{catalog, system, Change, Target, Tweaks};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Plan {
    Balanced,
    High,
    Ultimate,
}

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub warning: Option<&'static str>,
    /// Tweak id and the state it should end up in (`Default` = Windows default).
    pub tweaks: &'static [(&'static str, Target)],
    pub plan: Plan,
}

#[cfg(test)]
const ALL_GAMING: &[&str] = &[
    "game.game_mode",
    "game.no_background_recording",
    "game.windowed_optimizations",
    "game.mouse_acceleration_off",
    "game.sticky_keys_prompt_off",
    "game.games_priority",
    "game.system_responsiveness",
    "game.network_throttling_off",
    "perf.power_throttling",
    "perf.transparency",
    "perf.window_animations",
    "game.toggle_keys_prompt_off",
    "game.filter_keys_prompt_off",
    "game.gamebar_tips",
    "game.priority_separation",
];

pub static PRESETS: &[Preset] = &[
    Preset {
        id: "balanced",
        name: "Balanced",
        description: "Everyday use. Windows defaults with Game Mode on and background recording off, on the Balanced power plan.",
        warning: None,
        tweaks: &[
            ("game.game_mode", Target::On),
            ("game.no_background_recording", Target::On),
            ("game.windowed_optimizations", Target::Default),
            ("game.mouse_acceleration_off", Target::Default),
            ("game.sticky_keys_prompt_off", Target::Default),
            ("game.games_priority", Target::Default),
            ("game.system_responsiveness", Target::Default),
            ("game.network_throttling_off", Target::Default),
            ("perf.power_throttling", Target::Default),
            ("perf.transparency", Target::Default),
            ("perf.window_animations", Target::Default),
            ("game.priority_separation", Target::Default),
            ("game.toggle_keys_prompt_off", Target::Default),
            ("game.filter_keys_prompt_off", Target::Default),
            ("game.gamebar_tips", Target::Default),
        ],
        plan: Plan::Balanced,
    },
    Preset {
        id: "gaming",
        name: "Gaming",
        description: "Game Mode, no background recording, flip-model presentation for windowed games, raw mouse input, no Sticky/Toggle/Filter Keys pop-ups or Game Bar tips, and the High performance power plan.",
        warning: None,
        tweaks: &[
            ("game.game_mode", Target::On),
            ("game.no_background_recording", Target::On),
            ("game.windowed_optimizations", Target::On),
            ("game.mouse_acceleration_off", Target::On),
            ("game.sticky_keys_prompt_off", Target::On),
            ("game.toggle_keys_prompt_off", Target::On),
            ("game.filter_keys_prompt_off", Target::On),
            ("game.gamebar_tips", Target::On),
        ],
        plan: Plan::High,
    },
    Preset {
        id: "maximum",
        name: "Maximum Performance",
        description: "Everything in Gaming, plus multimedia-scheduler priority for games, short foreground-favouring CPU time slices, less CPU reserved for background work, no network or power throttling, fewer visual effects, and the Ultimate Performance power plan.",
        warning: Some(
            "Uses noticeably more power and produces more heat; on laptops battery life drops. Background apps, audio and downloads get less priority while you play. The extra gains over the Gaming preset are small and vary by game; measure with the FPS monitor before and after.",
        ),
        tweaks: &[
            ("game.game_mode", Target::On),
            ("game.no_background_recording", Target::On),
            ("game.windowed_optimizations", Target::On),
            ("game.mouse_acceleration_off", Target::On),
            ("game.sticky_keys_prompt_off", Target::On),
            ("game.games_priority", Target::On),
            ("game.system_responsiveness", Target::On),
            ("game.network_throttling_off", Target::On),
            ("perf.power_throttling", Target::On),
            ("perf.transparency", Target::On),
            ("perf.window_animations", Target::On),
            ("game.toggle_keys_prompt_off", Target::On),
            ("game.filter_keys_prompt_off", Target::On),
            ("game.gamebar_tips", Target::On),
            ("game.priority_separation", Target::On),
        ],
        plan: Plan::Ultimate,
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetChange {
    pub id: &'static str,
    pub title: &'static str,
    pub caveat: Option<&'static str>,
    pub from: State,
    pub to: State,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub warning: Option<&'static str>,
    pub plan: Plan,
    /// Every listed tweak already matches and the plan is active.
    pub active: bool,
    pub changes: Vec<PresetChange>,
    pub plan_change: Option<String>,
    pub requires_admin: bool,
}

/// What the last preset changed, so it can be reverted in one step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastPreset {
    pub preset: String,
    pub at: u64,
    pub tweaks: Vec<String>,
    pub previous_plan: Option<String>,
    pub previous_plan_name: Option<String>,
}

fn target_state(t: &crate::tweaks::model::Tweak, target: Target) -> State {
    match target {
        Target::On => State::On,
        Target::Off => State::Off,
        Target::Default | Target::Undo => t.default_state(),
    }
}

fn plan_matches(plan: Plan, active_name: &str, active_guid: &str) -> bool {
    match plan {
        Plan::Balanced => active_guid == "381b4222-f694-41f0-9685-ff5bb260df2e",
        Plan::High => active_guid == system::HIGH_PERFORMANCE || active_name.to_lowercase().contains("high performance"),
        Plan::Ultimate => active_guid == system::ULTIMATE_PERFORMANCE || active_name.to_lowercase().contains("ultimate"),
    }
}

/// Switches to the plan, creating High/Ultimate Performance if the PC doesn't list it yet.
fn plan_script(plan: Plan) -> String {
    match plan {
        Plan::Balanced => "powercfg /setactive 381b4222-f694-41f0-9685-ff5bb260df2e\n'Power plan: Balanced'\n".into(),
        Plan::High | Plan::Ultimate => {
            let (guid, pattern, label) = if plan == Plan::High {
                (system::HIGH_PERFORMANCE, "(?i)high performance", "High performance")
            } else {
                (system::ULTIMATE_PERFORMANCE, "(?i)ultimate", "Ultimate Performance")
            };
            format!(
                r#"$list = powercfg /list
$line = $list | Where-Object {{ $_ -match '{guid}' -or $_ -match '{pattern}' }} | Select-Object -First 1
if ($line -match '([0-9a-fA-F]{{8}}-[0-9a-fA-F-]{{27}})') {{ $g = $Matches[1] }}
else {{
    $out = powercfg -duplicatescheme {guid} 2>&1
    if ("$out" -match '([0-9a-fA-F]{{8}}-[0-9a-fA-F-]{{27}})') {{ $g = $Matches[1] }}
}}
if ($g) {{ powercfg /setactive $g; 'Power plan: {label}' }}
else {{ 'This PC does not offer the {label} plan (common with Modern Standby); keeping the current plan.' }}
"#
            )
        }
    }
}

pub struct Gaming {
    tweaks: Arc<Tweaks>,
    path: PathBuf,
    last: Arc<Mutex<Option<LastPreset>>>,
}

impl Gaming {
    pub fn new(tweaks: Arc<Tweaks>, data_dir: PathBuf) -> Self {
        let path = data_dir.join("last-preset.json");
        let last = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok());
        Self { tweaks, path, last: Arc::new(Mutex::new(last)) }
    }

    pub fn last(&self) -> Option<LastPreset> {
        self.last.lock().ok().and_then(|l| l.clone())
    }

    pub fn presets(&self) -> AppResult<Vec<PresetInfo>> {
        crate::tweaks::refresh_states();
        let info = system::info()?;
        let active = info.plans.iter().find(|p| p.active);
        let (aname, aguid) = active.map(|p| (p.name.as_str(), p.guid.as_str())).unwrap_or(("", ""));
        Ok(PRESETS
            .iter()
            .map(|p| {
                let changes: Vec<PresetChange> = p
                    .tweaks
                    .iter()
                    .filter_map(|(id, target)| {
                        let t = catalog::find(id)?;
                        let (from, to) = (t.state(), target_state(t, *target));
                        (from != to).then_some(PresetChange { id: t.id, title: t.title, caveat: t.caveat, from, to })
                    })
                    .collect();
                let plan_ok = plan_matches(p.plan, aname, aguid);
                let requires_admin = changes.iter().any(|c| catalog::find(c.id).is_some_and(|t| t.needs_admin()));
                PresetInfo {
                    id: p.id,
                    name: p.name,
                    description: p.description,
                    warning: p.warning,
                    plan: p.plan,
                    active: changes.is_empty() && plan_ok,
                    plan_change: (!plan_ok).then(|| format!("{} → {}", if aname.is_empty() { "current plan" } else { aname }, match p.plan {
                        Plan::Balanced => "Balanced",
                        Plan::High => "High performance",
                        Plan::Ultimate => "Ultimate Performance",
                    })),
                    requires_admin,
                    changes,
                }
            })
            .collect())
    }

    pub fn apply_preset(&self, id: &str) -> AppResult<JobRequest> {
        let preset = PRESETS.iter().find(|p| p.id == id).ok_or_else(|| AppError::Other("Unknown preset.".into()))?;
        let info = self.presets()?.into_iter().find(|p| p.id == id).expect("preset listed");
        let previous = system::info()?.plans.into_iter().find(|p| p.active);
        let changes: Vec<Change> = info
            .changes
            .iter()
            .map(|c| Change { id: c.id.into(), target: preset.tweaks.iter().find(|(t, _)| *t == c.id).map(|(_, t)| *t).unwrap_or(Target::On) })
            .collect();

        let plan = plan_script(preset.plan);
        let mut req = if changes.is_empty() {
            let mut r = JobRequest::internal("", crate::engine::types::Shell::PowerShell, plan.clone());
            r.timeout = std::time::Duration::from_secs(120);
            r
        } else {
            let mut r = self.tweaks.apply(&changes)?;
            r.command = format!("{plan}{}", r.command);
            r
        };
        req.title = format!("Apply {} preset", preset.name);
        req.operation_id = Some("gaming.preset".into());

        // Chain the tweak snapshot callback with recording what this preset changed.
        let inner = req.on_complete.take();
        let record = LastPreset {
            preset: preset.id.into(),
            at: now_ms(),
            tweaks: changes.iter().map(|c| c.id.clone()).collect(),
            previous_plan: previous.as_ref().map(|p| p.guid.clone()),
            previous_plan_name: previous.map(|p| p.name),
        };
        let store = self.last.clone();
        let path = self.path.clone();
        req.on_complete = Some(Callback(Arc::new(move |e| {
            if let Some(cb) = &inner {
                (cb.0)(e);
            }
            if matches!(e.status, JobStatus::Succeeded | JobStatus::Failed) {
                if let Ok(mut l) = store.lock() {
                    *l = Some(record.clone());
                }
                if let Some(d) = path.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                let _ = std::fs::write(&path, serde_json::to_string_pretty(&record).unwrap_or_default());
            }
        })));
        Ok(req)
    }

    /// Undoes every tweak the last preset changed and switches back to the previous power plan.
    pub fn revert_last(&self) -> AppResult<JobRequest> {
        let last = self.last().ok_or_else(|| AppError::Other("No preset has been applied yet.".into()))?;
        let undoable: Vec<Change> = self
            .tweaks
            .list()
            .into_iter()
            .filter(|t| t.can_undo && last.tweaks.iter().any(|id| id == t.id))
            .map(|t| Change { id: t.id.into(), target: Target::Undo })
            .collect();
        let plan = match &last.previous_plan {
            Some(g) if g.len() == 36 && g.chars().all(|c| c.is_ascii_hexdigit() || c == '-') => {
                format!("powercfg /setactive {g}\n'Power plan: {}'\n", last.previous_plan_name.as_deref().unwrap_or(g).replace('\'', "''"))
            }
            _ => String::new(),
        };
        let mut req = if undoable.is_empty() {
            let mut r = JobRequest::internal("", crate::engine::types::Shell::PowerShell, plan.clone());
            r.timeout = std::time::Duration::from_secs(120);
            r
        } else {
            let mut r = self.tweaks.apply(&undoable)?;
            r.command = format!("{plan}{}", r.command);
            r
        };
        req.title = "Revert last gaming preset".into();
        req.operation_id = Some("gaming.revert".into());
        let inner = req.on_complete.take();
        let store = self.last.clone();
        let path = self.path.clone();
        req.on_complete = Some(Callback(Arc::new(move |e| {
            if let Some(cb) = &inner {
                (cb.0)(e);
            }
            if e.status == JobStatus::Succeeded {
                if let Ok(mut l) = store.lock() {
                    *l = None;
                }
                let _ = std::fs::remove_file(&path);
            }
        })));
        Ok(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_reference_real_tweaks() {
        for id in ALL_GAMING {
            assert!(catalog::find(id).is_some(), "{id}");
        }
        for p in PRESETS {
            for (id, _) in p.tweaks {
                assert!(ALL_GAMING.contains(id), "{} uses {id}", p.id);
            }
        }
        assert!(PRESETS.iter().find(|p| p.id == "maximum").unwrap().warning.is_some(), "Maximum Performance must warn");
    }

    #[test]
    fn preview_lists_changes() {
        let dir = std::env::temp_dir().join(format!("blob-gaming-test-{}", std::process::id()));
        let g = Gaming::new(Arc::new(Tweaks::new(dir.clone())), dir.clone());
        let presets = g.presets().expect("presets");
        assert_eq!(presets.len(), 3);
        for p in &presets {
            assert!(p.active || !p.changes.is_empty() || p.plan_change.is_some(), "{} is inactive but lists nothing to change", p.id);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_games_displays_and_profiles() {
        let dir = std::env::temp_dir().join(format!("blob-games-test-{}", std::process::id()));
        let g = games::Games::new(dir.clone());
        let list = g.list().expect("game detection");
        println!("games: {:?}", list.iter().map(|x| (&x.name, &x.exe)).collect::<Vec<_>>());
        if let Some(game) = list.iter().find(|x| x.source == "steam") {
            let c = g.candidates(&game.id).expect("candidates");
            println!("{} candidates: {:?}", game.name, c.iter().take(3).map(|c| &c.path).collect::<Vec<_>>());
        }
        // Profiles read without touching anything.
        if let Some(exe) = list.iter().find_map(|x| x.exe.clone()) {
            let _ = games::read_profile(&exe);
        }
        assert!(g.add_custom("Bad", r"C:\Windows\notepad.txt").is_err());
        assert!(g.add_custom("Bad", "relative.exe").is_err());

        let displays = display::list().expect("displays");
        assert!(!displays.is_empty());
        assert!(displays.iter().all(|d| d.rates.contains(&d.hz) || d.rates.is_empty()));
        assert!(display::set_refresh_rate(&displays[0].device, 7).is_err(), "unsupported rate refused");
        let _ = std::fs::remove_dir_all(dir);
    }
}
