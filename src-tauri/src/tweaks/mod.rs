//! Windows tweaks: catalogue, live state, apply / reset-to-default / undo, with a registry snapshot
//! taken before every change.

pub mod catalog;
pub mod model;
pub mod services;
pub mod startup;
pub mod system;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::types::{now_ms, Callback, JobRequest, JobStatus, Shell};
use crate::error::{AppError, AppResult};
use crate::util::registry::{self, Hive, RegData};
use model::{After, Category, Op, Recommend, StartType, State, Tweak};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakInfo {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub caveat: Option<&'static str>,
    pub category: Category,
    pub group: &'static str,
    pub recommend: Recommend,
    pub after: After,
    pub requires_admin: bool,
    pub state: State,
    /// The current state is what Windows ships with.
    pub at_default: bool,
    pub default_on: bool,
    /// Exact changes, shown under "Details".
    pub changes_on: Vec<String>,
    pub changes_off: Vec<String>,
    /// A snapshot from before the last change exists, so it can be undone.
    pub can_undo: bool,
    /// When that snapshot was taken (ms since epoch).
    pub last_change_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    /// Apply the `on` list.
    On,
    /// Apply the `off` list (differs from Default only for default-on tweaks such as Game Mode).
    Off,
    /// Restore the Windows default.
    Default,
    /// Put back exactly what was there before the last change made here.
    Undo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Change {
    pub id: String,
    pub target: Target,
}

/// Previous state of every location a tweak touches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Saved {
    Value { hive: Hive, path: String, name: String, data: Option<RegData> },
    Key { hive: Hive, path: String, existed: bool },
    Service { name: String, start: Option<u32> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    at: u64,
    entries: Vec<Saved>,
}

fn snapshot(t: &Tweak) -> Snapshot {
    let mut entries = Vec::new();
    for op in t.on.iter().chain(t.off) {
        let e = match *op {
            Op::Set(h, p, n, _) | Op::Opt(h, p, n, _) | Op::Del(h, p, n) => {
                Saved::Value { hive: h, path: p.into(), name: n.into(), data: registry::read(h, p, n) }
            }
            Op::DelKey(h, p) => Saved::Key { hive: h, path: p.into(), existed: registry::key_exists(h, p) },
            Op::Svc(name, _) => Saved::Service {
                name: name.into(),
                start: model::service_start(name).map(StartType::registry_value),
            },
        };
        let dup = entries.iter().any(|x: &Saved| match (x, &e) {
            (Saved::Value { hive: a, path: b, name: c, .. }, Saved::Value { hive: d, path: f, name: g, .. }) => a == d && b == f && c == g,
            (Saved::Key { hive: a, path: b, .. }, Saved::Key { hive: d, path: f, .. }) => a == d && b == f,
            (Saved::Service { name: a, .. }, Saved::Service { name: b, .. }) => a == b,
            _ => false,
        });
        if !dup {
            entries.push(e);
        }
    }
    Snapshot { at: now_ms(), entries }
}

fn restore_script(s: &Snapshot) -> String {
    let mut out = String::new();
    // Keys first (a removed key also removes the values below it), then values.
    for e in &s.entries {
        if let Saved::Key { hive, path, existed: false } = e {
            out.push_str(&registry::ps_delete_key(*hive, path));
        }
    }
    for e in &s.entries {
        match e {
            Saved::Value { hive, path, name, data: Some(d) } => out.push_str(&registry::ps_set(*hive, path, name, d)),
            Saved::Value { hive, path, name, data: None } => out.push_str(&registry::ps_delete_value(*hive, path, name)),
            Saved::Service { name, start: Some(v) } => {
                if let Some(st) = StartType::from_registry(*v) {
                    out.push_str(&model::service_script(name, st));
                }
            }
            _ => {}
        }
    }
    out
}

pub struct Tweaks {
    backups_path: PathBuf,
    backups: Arc<Mutex<HashMap<String, Vec<Snapshot>>>>,
    build: u32,
}

const KEEP_SNAPSHOTS: usize = 10;

impl Tweaks {
    pub fn new(data_dir: PathBuf) -> Self {
        let backups_path = data_dir.join("tweak-backups.json");
        let backups = std::fs::read_to_string(&backups_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let build = match registry::read(Hive::LocalMachine, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "CurrentBuildNumber") {
            Some(RegData::String(s)) => s.parse().unwrap_or(0),
            _ => 0,
        };
        Self { backups_path, backups: Arc::new(Mutex::new(backups)), build }
    }

    fn supported(&self, t: &Tweak) -> bool {
        self.build >= t.min_build
    }

    pub fn list(&self) -> Vec<TweakInfo> {
        let backups = self.backups.lock().map(|b| b.clone()).unwrap_or_default();
        catalog::TWEAKS
            .iter()
            .filter(|t| self.supported(t))
            .map(|t| {
                let state = t.state();
                TweakInfo {
                id: t.id,
                title: t.title,
                description: t.description,
                caveat: t.caveat,
                category: t.category,
                group: t.group,
                recommend: t.recommend,
                after: t.after,
                requires_admin: t.needs_admin(),
                state,
                at_default: state == t.default_state(),
                default_on: t.default_on,
                changes_on: t.on.iter().map(Op::describe).collect(),
                changes_off: t.off.iter().map(Op::describe).collect(),
                can_undo: backups.get(t.id).is_some_and(|v| !v.is_empty()),
                last_change_at: backups.get(t.id).and_then(|v| v.last()).map(|s| s.at),
            }})
            .collect()
    }

    /// Builds one job applying all changes; a snapshot of each tweak is saved once the job succeeds.
    pub fn apply(&self, changes: &[Change]) -> AppResult<JobRequest> {
        if changes.is_empty() {
            return Err(AppError::Other("Nothing to change.".into()));
        }
        let backups = self.backups.lock().map(|b| b.clone()).unwrap_or_default();
        let mut script = String::from("$failed = 0\n");
        let mut admin = false;
        let mut snapshots: Vec<(String, Snapshot)> = Vec::new();
        let mut undone: Vec<String> = Vec::new();
        let total = changes.len();

        for (i, c) in changes.iter().enumerate() {
            let t = catalog::find(&c.id).filter(|t| self.supported(t)).ok_or_else(|| AppError::Other(format!("Unknown tweak '{}'", c.id)))?;
            admin |= t.needs_admin();
            let (verb, body) = match c.target {
                Target::On => ("Applying", t.on.iter().map(Op::script).collect::<String>()),
                Target::Off => ("Turning off", t.off.iter().map(Op::script).collect::<String>()),
                Target::Default => ("Restoring Windows default for", t.default_ops().iter().map(Op::script).collect::<String>()),
                Target::Undo => {
                    let snap = backups
                        .get(t.id)
                        .and_then(|v| v.last())
                        .ok_or_else(|| AppError::Other(format!("Nothing to undo for '{}'", t.title)))?;
                    undone.push(t.id.to_string());
                    ("Undoing last change to", restore_script(snap))
                }
            };
            if c.target != Target::Undo {
                snapshots.push((t.id.to_string(), snapshot(t)));
            }
            let title = t.title.replace('\'', "''");
            script.push_str(&format!(
                "'##PROGRESS {} {verb} {title}'\n'{verb}: {title}'\ntry {{\n{body}}} catch {{ $failed++; [Console]::Error.WriteLine(\"  Failed: $($_.Exception.Message)\") }}\n",
                i * 100 / total
            ));
        }
        script.push_str("'##PROGRESS 100 Done'\nif ($failed) { exit 1 }\n");

        let title = if total == 1 {
            let t = catalog::find(&changes[0].id).map(|t| t.title).unwrap_or("tweak");
            match changes[0].target {
                Target::On => format!("Apply: {t}"),
                Target::Off => format!("Turn off: {t}"),
                Target::Default => format!("Reset to default: {t}"),
                Target::Undo => format!("Undo: {t}"),
            }
        } else {
            format!("Apply {total} tweaks")
        };

        let store = self.backups.clone();
        let path = self.backups_path.clone();
        let mut req = JobRequest::internal(title, Shell::PowerShell, script);
        req.operation_id = Some("tweaks.apply".into());
        req.requires_admin = admin;
        req.timeout = Duration::from_secs(10 * 60);
        req.on_complete = Some(Callback(Arc::new(move |entry| {
            // Snapshots are kept even when some tweaks failed: they record the state before this attempt.
            if entry.status == JobStatus::Declined || entry.status == JobStatus::Cancelled {
                return;
            }
            let Ok(mut b) = store.lock() else { return };
            for id in &undone {
                if entry.status == JobStatus::Succeeded {
                    if let Some(v) = b.get_mut(id) {
                        v.pop();
                    }
                }
            }
            for (id, snap) in &snapshots {
                let v = b.entry(id.clone()).or_default();
                v.push(snap.clone());
                if v.len() > KEEP_SNAPSHOTS {
                    v.remove(0);
                }
            }
            if let Ok(json) = serde_json::to_string_pretty(&*b) {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(&path, json);
            }
        })));
        Ok(req)
    }
}

#[cfg(test)]
mod tests;
