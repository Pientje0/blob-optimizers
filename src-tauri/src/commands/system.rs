use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::system::hardware::{self, HardwareInfo};
use crate::system::live::LiveStats;

#[tauri::command]
pub async fn get_live_stats(state: State<'_, AppState>) -> AppResult<LiveStats> {
    let mut monitor = state
        .monitor
        .lock()
        .map_err(|_| AppError::Other("monitor lock poisoned".into()))?;
    Ok(monitor.sample())
}

/// Returns cached hardware info, or detects it (a few seconds) when missing or `refresh` is set.
#[tauri::command]
pub async fn get_hardware_info(
    state: State<'_, AppState>,
    refresh: bool,
) -> AppResult<HardwareInfo> {
    if !refresh {
        if let Some(info) = state.hardware.lock().ok().and_then(|g| g.clone()) {
            return Ok(info);
        }
    }
    let info = tauri::async_runtime::spawn_blocking(hardware::detect)
        .await
        .map_err(|e| AppError::Other(e.to_string()))??;
    if let Ok(mut cache) = state.hardware.lock() {
        *cache = Some(info.clone());
    }
    Ok(info)
}
