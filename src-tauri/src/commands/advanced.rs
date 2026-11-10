use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::backup::{self, ImportChange, ProfileFile, ProtectionStatus, RegBackup};
use crate::engine::types::{HistoryEntry, JobRequest};
use crate::engine::Engine;
use crate::error::{AppError, AppResult};
use crate::network::{self, dns, Adapter, Check};
use crate::settings::{Settings, SettingsStore};
use crate::tweaks::Tweaks;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::Other(e.to_string()))?
}

async fn submit(engine: &Engine, build: impl FnOnce() -> AppResult<JobRequest> + Send + 'static) -> AppResult<HistoryEntry> {
    let req = blocking(build).await?;
    engine.submit(req)
}

fn data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path().app_local_data_dir().map_err(|e| AppError::Other(e.to_string()))
}

fn registry_backup_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(data_dir(app)?.join("apps").join("backups").join("registry"))
}

fn profiles_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app.path().document_dir().map_err(|e| AppError::Other(e.to_string()))?.join("Blob Optimizers"))
}

// ---------------- Network ----------------

#[tauri::command]
pub async fn network_adapters() -> AppResult<Vec<Adapter>> {
    blocking(network::adapters).await
}

#[tauri::command]
pub async fn network_set_dns(engine: State<'_, Engine>, index: u32, servers: Vec<String>) -> AppResult<HistoryEntry> {
    submit(&engine, move || network::set_dns(index, &servers)).await
}

#[tauri::command]
pub async fn network_set_adapter(engine: State<'_, Engine>, index: u32, enabled: bool) -> AppResult<HistoryEntry> {
    submit(&engine, move || network::set_adapter_enabled(index, enabled)).await
}

#[tauri::command]
pub async fn network_set_category(engine: State<'_, Engine>, index: u32, private: bool) -> AppResult<HistoryEntry> {
    submit(&engine, move || network::set_category(index, private)).await
}

#[tauri::command]
pub async fn network_diagnose() -> AppResult<Vec<Check>> {
    blocking(network::diagnose).await
}

#[tauri::command]
pub async fn network_dns_presets() -> Vec<(&'static str, &'static str, &'static str)> {
    dns::RESOLVERS.to_vec()
}

#[tauri::command]
pub async fn network_dns_benchmark(current: Vec<String>) -> AppResult<Vec<dns::BenchResult>> {
    let ips: Vec<IpAddr> = current.iter().filter_map(|s| s.parse().ok()).collect();
    blocking(move || Ok(dns::benchmark(&ips))).await
}

// ---------------- Restore & backup ----------------

#[tauri::command]
pub async fn restore_status() -> ProtectionStatus {
    backup::protection_status()
}

#[tauri::command]
pub async fn restore_list(engine: State<'_, Engine>) -> AppResult<HistoryEntry> {
    engine.submit(backup::list_restore_points())
}

#[tauri::command]
pub async fn restore_rollback(engine: State<'_, Engine>, sequence: u32) -> AppResult<HistoryEntry> {
    engine.submit(backup::restore_to(sequence))
}

#[tauri::command]
pub async fn backups_registry(app: AppHandle) -> AppResult<Vec<RegBackup>> {
    let dir = registry_backup_dir(&app)?;
    Ok(backup::registry_backups(&dir))
}

#[tauri::command]
pub async fn backups_registry_import(app: AppHandle, engine: State<'_, Engine>, file: String) -> AppResult<HistoryEntry> {
    let dir = registry_backup_dir(&app)?;
    submit(&engine, move || backup::import_registry_backup(&dir, &file)).await
}

#[tauri::command]
pub async fn profile_export(app: AppHandle, tweaks: State<'_, Arc<Tweaks>>) -> AppResult<String> {
    let dir = profiles_dir(&app)?;
    let t = tweaks.inner().clone();
    let version = app.package_info().version.to_string();
    blocking(move || backup::export_profile(&t, &dir, &version)).await
}

#[tauri::command]
pub async fn profile_list(app: AppHandle) -> AppResult<(String, Vec<ProfileFile>)> {
    let dir = profiles_dir(&app)?;
    Ok((dir.to_string_lossy().into_owned(), backup::list_profiles(&dir)))
}

#[tauri::command]
pub async fn profile_preview(app: AppHandle, tweaks: State<'_, Arc<Tweaks>>, file: String) -> AppResult<Vec<ImportChange>> {
    let dir = profiles_dir(&app)?;
    let t = tweaks.inner().clone();
    blocking(move || backup::preview_profile(&t, &dir, &file)).await
}

// ---------------- Settings ----------------

#[tauri::command]
pub async fn settings_get(store: State<'_, Arc<SettingsStore>>) -> AppResult<Settings> {
    Ok(store.get())
}

/// Saves settings; returns the job that changes "start with Windows" when that setting changed.
#[tauri::command]
pub async fn settings_set(store: State<'_, Arc<SettingsStore>>, engine: State<'_, Engine>, settings: Settings) -> AppResult<Option<HistoryEntry>> {
    let before = store.get();
    store.save(&settings)?;
    if before.history_limit != settings.history_limit {
        engine.set_history_limit(settings.history_limit);
    }
    if before.start_with_windows != settings.start_with_windows {
        return Ok(Some(engine.submit(store.start_with_windows(settings.start_with_windows)?)?));
    }
    Ok(None)
}

// ---------------- Updates ----------------

#[tauri::command]
pub async fn updates_check(app: AppHandle, store: State<'_, Arc<SettingsStore>>) -> AppResult<crate::updates::UpdateInfo> {
    let current = app.package_info().version.to_string();
    let repo = store.get().update_repo;
    blocking(move || crate::updates::check(&current, &repo)).await
}

#[tauri::command]
pub async fn updates_open(store: State<'_, Arc<SettingsStore>>, url: String) -> AppResult<()> {
    crate::updates::open_release_page(&store.get().update_repo, &url)
}

#[tauri::command]
pub async fn app_paths(app: AppHandle) -> AppResult<(String, String)> {
    Ok((data_dir(&app)?.to_string_lossy().into_owned(), profiles_dir(&app)?.to_string_lossy().into_owned()))
}
