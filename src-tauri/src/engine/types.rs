use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    PowerShell,
    Cmd,
}

/// Where a job came from. User-typed commands are validated separately from reviewed catalog operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// A predefined, reviewed operation from the catalog.
    Catalog,
    /// A command typed by the user in the Terminal tab.
    User,
    /// Issued by another module of the app (tweaks, debloater, ...).
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    /// The UAC prompt was declined.
    Declined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Out,
    Err,
    /// Messages from the engine itself (start/finish banners, warnings).
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputLine {
    pub stream: Stream,
    pub text: String,
    /// The line was ended by a bare carriage return (in-place progress update); the next line replaces it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub transient: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub percent: f32,
    pub message: Option<String>,
}

/// A job as recorded in the history log. Also the payload of `engine:job` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub title: String,
    pub source: Source,
    pub operation_id: Option<String>,
    pub shell: Shell,
    /// The exact command text that was executed (shown to the user for transparency).
    pub command: String,
    pub elevated: bool,
    pub modifies_system: bool,
    pub cancellable: bool,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub status: JobStatus,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub progress: Option<Progress>,
    pub line_count: usize,
}

/// Called on the job thread after a job finishes, before the final `engine:job` event is emitted,
/// so state it updates is visible to the UI by the time the UI hears the job is done.
#[derive(Clone)]
pub struct Callback(pub std::sync::Arc<dyn Fn(&HistoryEntry) + Send + Sync>);

impl std::fmt::Debug for Callback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Callback")
    }
}

/// Everything the engine needs to run something.
#[derive(Debug, Clone)]
pub struct JobRequest {
    pub title: String,
    pub source: Source,
    pub operation_id: Option<String>,
    pub shell: Shell,
    pub command: String,
    pub requires_admin: bool,
    pub modifies_system: bool,
    pub cancellable: bool,
    pub timeout: Duration,
    /// Regex with one capture group holding a percentage, applied to output lines.
    pub progress_pattern: Option<&'static str>,
    /// On cancel/timeout, also kill processes the job started. False when those are independent
    /// programs (e.g. a vendor uninstaller) that must not be killed midway; only our waiter stops.
    pub kill_tree: bool,
    pub on_complete: Option<Callback>,
}

impl JobRequest {
    /// A request issued by an app module, with conservative defaults.
    pub fn internal(title: impl Into<String>, shell: Shell, command: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            source: Source::Internal,
            operation_id: None,
            shell,
            command: command.into(),
            requires_admin: false,
            modifies_system: true,
            cancellable: false,
            timeout: std::time::Duration::from_secs(30 * 60),
            progress_pattern: None,
            kill_tree: true,
            on_complete: None,
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
