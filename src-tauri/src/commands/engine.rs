use std::collections::HashMap;
use std::time::Duration;

use tauri::State;

use crate::engine::catalog::{self, Operation};
use crate::engine::types::{HistoryEntry, JobRequest, OutputLine, Shell, Source};
use crate::engine::validate::{self, Level, Validation};
use crate::engine::{ActiveJob, Engine};
use crate::error::{AppError, AppResult};

/// Time limit for commands typed in the Terminal tab.
const USER_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[tauri::command]
pub async fn engine_catalog() -> Vec<Operation> {
    catalog::OPERATIONS.to_vec()
}

#[tauri::command]
pub async fn engine_run_operation(
    engine: State<'_, Engine>,
    id: String,
    params: Option<HashMap<String, String>>,
) -> AppResult<HistoryEntry> {
    let req = catalog::request(&id, &params.unwrap_or_default())?;
    engine.submit(req)
}

#[tauri::command]
pub async fn engine_validate(shell: Shell, command: String) -> Validation {
    validate::validate(shell, &command)
}

/// Runs a command typed by the user. Re-validates server-side: blocked commands never run and
/// risky ones require `confirmed` (the UI shows the reasons first).
#[tauri::command]
pub async fn engine_run_command(
    engine: State<'_, Engine>,
    shell: Shell,
    command: String,
    confirmed: bool,
    as_admin: bool,
) -> AppResult<HistoryEntry> {
    let v = validate::validate(shell, &command);
    match v.level {
        Level::Block => return Err(AppError::Other(format!("Blocked for safety: {}", v.reasons.join(" ")))),
        Level::Warn if !confirmed => {
            return Err(AppError::Other(format!("Confirmation required: {}", v.reasons.join(" "))))
        }
        _ => {}
    }
    let command = command.trim().to_string();
    let title: String = command.lines().next().unwrap_or_default().chars().take(80).collect();
    engine.submit(JobRequest {
        title,
        source: Source::User,
        operation_id: None,
        shell,
        command,
        requires_admin: as_admin,
        modifies_system: v.level == Level::Warn,
        cancellable: true,
        timeout: USER_TIMEOUT,
        progress_pattern: None,
        kill_tree: true,
        on_complete: None,
    })
}

#[tauri::command]
pub async fn engine_cancel(engine: State<'_, Engine>, id: String) -> AppResult<()> {
    engine.cancel(&id)
}

#[tauri::command]
pub async fn engine_active(engine: State<'_, Engine>) -> AppResult<Vec<ActiveJob>> {
    Ok(engine.active())
}

#[tauri::command]
pub async fn engine_output(engine: State<'_, Engine>, id: String) -> AppResult<Vec<OutputLine>> {
    engine.output(&id)
}

#[tauri::command]
pub async fn engine_history(engine: State<'_, Engine>, limit: Option<usize>) -> AppResult<Vec<HistoryEntry>> {
    Ok(engine.history(limit.unwrap_or(200)))
}

#[tauri::command]
pub async fn engine_clear_history(engine: State<'_, Engine>) -> AppResult<()> {
    engine.clear_history()
}

/// Restarts Blob Optimizers with administrator rights (one UAC prompt instead of one per operation).
#[tauri::command]
pub async fn relaunch_as_admin(app: tauri::AppHandle, engine: State<'_, Engine>) -> AppResult<()> {
    if engine.is_elevated() {
        return Err(AppError::Other("Blob Optimizers is already running as administrator.".into()));
    }
    if cfg!(debug_assertions) {
        return Err(AppError::Other(
            "Relaunching is only available in installed builds. In development, start `npm run tauri dev` from an administrator terminal.".into(),
        ));
    }
    if !engine.active().is_empty() {
        return Err(AppError::Other("Wait for running operations to finish before relaunching.".into()));
    }
    #[cfg(windows)]
    {
        use crate::engine::launch_elevated_self;
        launch_elevated_self()?;
        app.exit(0);
    }
    Ok(())
}
