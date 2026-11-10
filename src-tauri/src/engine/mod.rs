//! Command engine: every system operation runs through here.
//!
//! - PowerShell and CMD, hidden, with live output streamed to the UI as `engine:output` events
//! - Job lifecycle (`engine:job` events), progress, cancellation, timeouts
//! - UAC elevation per operation when the app itself is not elevated
//! - Conflict prevention: only one system-modifying operation at a time
//! - Persistent history and per-job output logs

pub mod catalog;
mod decoder;
mod history;
mod launch;
mod runner;
#[cfg(test)]
mod tests;
pub mod types;
pub mod validate;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::error::{AppError, AppResult};
use history::History;
use types::*;

/// Maximum number of output lines kept in memory per running job (the log file keeps everything).
const MEMORY_LINES: usize = 5_000;
const MAX_CONCURRENT: usize = 4;

pub type Emitter = Box<dyn Fn(&str, serde_json::Value) + Send + Sync>;

pub(crate) struct Job {
    pub entry: Mutex<HistoryEntry>,
    pub lines: Mutex<VecDeque<OutputLine>>,
    pub cancel: AtomicBool,
    /// Exclusive jobs (system modifications, user commands) never run alongside each other.
    pub exclusive: bool,
}

pub(crate) struct Inner {
    emit: Emitter,
    jobs: Mutex<HashMap<String, Arc<Job>>>,
    history: History,
    data_dir: PathBuf,
    counter: AtomicU64,
    app_elevated: bool,
}

impl Inner {
    pub fn emit<T: Serialize>(&self, event: &str, payload: &T) {
        if let Ok(v) = serde_json::to_value(payload) {
            (self.emit)(event, v);
        }
    }

    pub fn log_path(&self, id: &str) -> PathBuf {
        self.data_dir.join("jobs").join(format!("{id}.jsonl"))
    }

    pub fn run_dir(&self) -> PathBuf {
        self.data_dir.join("run")
    }
}

/// Starts a new elevated instance of this executable (UAC prompt). The caller exits afterwards.
#[cfg(windows)]
pub fn launch_elevated_self() -> AppResult<()> {
    let exe = std::env::current_exe()?;
    launch_elevated_with_args(&exe.to_string_lossy(), "")
}

/// Starts `program args` through UAC without waiting for it.
#[cfg(windows)]
pub fn launch_elevated_with_args(program: &str, args: &str) -> AppResult<()> {
    launch::elevated::shell_execute_runas(program, args, false).map_err(|e| match e {
        launch::ElevateError::Declined => AppError::Other("Administrator permission was declined.".into()),
        launch::ElevateError::Failed(m) => AppError::Other(m),
    })?;
    Ok(())
}

#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveJob {
    pub entry: HistoryEntry,
    pub lines: Vec<OutputLine>,
}

impl Engine {
    pub fn new(data_dir: PathBuf, app_elevated: bool, emit: Emitter) -> Self {
        let _ = std::fs::create_dir_all(data_dir.join("jobs"));
        let _ = std::fs::create_dir_all(data_dir.join("run"));
        Self {
            inner: Arc::new(Inner {
                emit,
                jobs: Mutex::new(HashMap::new()),
                history: History::load(data_dir.join("history.jsonl"), data_dir.join("jobs")),
                data_dir,
                counter: AtomicU64::new(0),
                app_elevated,
            }),
        }
    }

    pub fn is_elevated(&self) -> bool {
        self.inner.app_elevated
    }

    /// Validates concurrency rules and starts the job on a background thread.
    pub fn submit(&self, req: JobRequest) -> AppResult<HistoryEntry> {
        let exclusive = req.modifies_system || req.source == Source::User;
        let id = format!("{}-{}", now_ms(), self.inner.counter.fetch_add(1, Ordering::Relaxed));
        let entry = HistoryEntry {
            id: id.clone(),
            title: req.title.clone(),
            source: req.source,
            operation_id: req.operation_id.clone(),
            shell: req.shell,
            command: req.command.clone(),
            elevated: req.requires_admin || self.inner.app_elevated,
            modifies_system: req.modifies_system,
            cancellable: req.cancellable,
            started_at: now_ms(),
            finished_at: None,
            status: JobStatus::Running,
            exit_code: None,
            error: None,
            progress: None,
            line_count: 0,
        };

        let job = Arc::new(Job {
            entry: Mutex::new(entry.clone()),
            lines: Mutex::new(VecDeque::new()),
            cancel: AtomicBool::new(false),
            exclusive,
        });

        {
            let mut jobs = self.inner.jobs.lock().map_err(|_| AppError::Other("engine lock poisoned".into()))?;
            if jobs.len() >= MAX_CONCURRENT {
                return Err(AppError::Other(format!(
                    "{MAX_CONCURRENT} operations are already running. Wait for one to finish."
                )));
            }
            if exclusive {
                if let Some(other) = jobs.values().find(|j| j.exclusive) {
                    let title = other.entry.lock().map(|e| e.title.clone()).unwrap_or_default();
                    return Err(AppError::Other(format!(
                        "\"{title}\" is still running. System changes run one at a time to avoid conflicts; wait for it to finish or cancel it."
                    )));
                }
            }
            jobs.insert(id, job.clone());
        }

        self.inner.emit("engine:job", &entry);
        let inner = self.inner.clone();
        std::thread::Builder::new()
            .name(format!("job-{}", entry.id))
            .spawn(move || runner::run(inner, job, req))
            .map_err(AppError::Io)?;
        Ok(entry)
    }

    /// Requests cancellation. Returns an error if the job is unknown or not safely cancellable.
    pub fn cancel(&self, id: &str) -> AppResult<()> {
        let jobs = self.inner.jobs.lock().map_err(|_| AppError::Other("engine lock poisoned".into()))?;
        let job = jobs.get(id).ok_or_else(|| AppError::Other("That operation is no longer running.".into()))?;
        let cancellable = job.entry.lock().map(|e| e.cancellable).unwrap_or(false);
        if !cancellable {
            return Err(AppError::Other(
                "This operation cannot be cancelled safely; stopping it midway could leave Windows in an inconsistent state.".into(),
            ));
        }
        job.cancel.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn active(&self) -> Vec<ActiveJob> {
        let jobs = match self.inner.jobs.lock() {
            Ok(j) => j,
            Err(_) => return Vec::new(),
        };
        let mut out: Vec<ActiveJob> = jobs
            .values()
            .map(|j| ActiveJob {
                entry: j.entry.lock().map(|e| e.clone()).expect("entry lock"),
                lines: j.lines.lock().map(|l| l.iter().cloned().collect()).unwrap_or_default(),
            })
            .collect();
        out.sort_by_key(|j| j.entry.started_at);
        out
    }

    /// Output of a running job (from memory) or a finished one (from its log file).
    pub fn output(&self, id: &str) -> AppResult<Vec<OutputLine>> {
        if let Some(job) = self.inner.jobs.lock().ok().and_then(|j| j.get(id).cloned()) {
            return Ok(job.lines.lock().map(|l| l.iter().cloned().collect()).unwrap_or_default());
        }
        if !self.inner.history.contains(id) {
            return Err(AppError::Other("Unknown operation.".into()));
        }
        let path = self.inner.log_path(id);
        let text = std::fs::read_to_string(&path).map_err(|_| AppError::Other("The output log for this operation is no longer available.".into()))?;
        Ok(text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
    }

    pub fn history(&self, limit: usize) -> Vec<HistoryEntry> {
        self.inner.history.recent(limit)
    }

    pub fn set_history_limit(&self, limit: usize) {
        self.inner.history.set_limit(limit);
    }

    pub fn clear_history(&self) -> AppResult<()> {
        self.inner.history.clear()
    }
}
