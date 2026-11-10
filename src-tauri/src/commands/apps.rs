use tauri::State;

use crate::apps::features::{self, Feature};
use crate::apps::inventory::InstalledApp;
use crate::apps::ledger::RemovedApp;
use crate::apps::leftovers::LeftoverScan;
use crate::apps::Apps;
use crate::engine::types::HistoryEntry;
use crate::engine::Engine;
use crate::error::{AppError, AppResult};

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn apps_inventory(apps: State<'_, std::sync::Arc<Apps>>, refresh: bool) -> AppResult<Vec<InstalledApp>> {
    let apps = apps.inner().clone();
    blocking(move || apps.inventory(refresh)).await
}

#[tauri::command]
pub async fn apps_uninstall(
    apps: State<'_, std::sync::Arc<Apps>>,
    engine: State<'_, Engine>,
    id: String,
) -> AppResult<HistoryEntry> {
    let apps = apps.inner().clone();
    let req = blocking(move || apps.uninstall_win32(&id)).await?;
    engine.submit(req)
}

#[tauri::command]
pub async fn apps_remove_store(
    apps: State<'_, std::sync::Arc<Apps>>,
    engine: State<'_, Engine>,
    ids: Vec<String>,
) -> AppResult<HistoryEntry> {
    let apps = apps.inner().clone();
    let req = blocking(move || apps.remove_appx(&ids)).await?;
    engine.submit(req)
}

#[tauri::command]
pub async fn apps_removed(apps: State<'_, std::sync::Arc<Apps>>) -> AppResult<Vec<RemovedApp>> {
    Ok(apps.ledger.list())
}

#[tauri::command]
pub async fn apps_reinstall(
    apps: State<'_, std::sync::Arc<Apps>>,
    engine: State<'_, Engine>,
    package_name: String,
) -> AppResult<HistoryEntry> {
    let req = apps.reinstall_appx(&package_name)?;
    engine.submit(req)
}

#[tauri::command]
pub async fn apps_scan_leftovers(apps: State<'_, std::sync::Arc<Apps>>, id: String) -> AppResult<LeftoverScan> {
    let apps = apps.inner().clone();
    blocking(move || apps.scan_leftovers(&id)).await
}

#[tauri::command]
pub async fn apps_cleanup_leftovers(
    apps: State<'_, std::sync::Arc<Apps>>,
    engine: State<'_, Engine>,
    scan_id: String,
    item_ids: Vec<String>,
) -> AppResult<HistoryEntry> {
    let req = apps.cleanup_leftovers(&scan_id, &item_ids)?;
    engine.submit(req)
}

#[tauri::command]
pub async fn features_list() -> AppResult<Vec<Feature>> {
    blocking(features::list).await
}

#[tauri::command]
pub async fn features_toggle(
    apps: State<'_, std::sync::Arc<Apps>>,
    engine: State<'_, Engine>,
    name: String,
    enable: bool,
) -> AppResult<HistoryEntry> {
    let apps = apps.inner().clone();
    let req = blocking(move || apps.toggle_feature(&name, enable)).await?;
    engine.submit(req)
}
