use std::sync::Arc;

use tauri::State;

use crate::engine::types::{HistoryEntry, JobRequest};
use crate::engine::Engine;
use crate::error::{AppError, AppResult};
use crate::system::processes::{ProcessInfo, ProcessManager};
use crate::tweaks::model::StartType;
use crate::tweaks::services::{ServiceInfo, Services};
use crate::tweaks::startup::{Startup, StartupList};
use crate::tweaks::system::{self, CleanupItem, SystemInfo, Timeouts};
use crate::tweaks::{Change, TweakInfo, Tweaks};

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::Other(e.to_string()))?
}

/// Builds a request off the UI thread (most builders read live system state) and submits it.
async fn submit(engine: &Engine, build: impl FnOnce() -> AppResult<JobRequest> + Send + 'static) -> AppResult<HistoryEntry> {
    let req = blocking(build).await?;
    engine.submit(req)
}

#[tauri::command]
pub async fn tweaks_list(tweaks: State<'_, Arc<Tweaks>>) -> AppResult<Vec<TweakInfo>> {
    let t = tweaks.inner().clone();
    blocking(move || Ok(t.list())).await
}

#[tauri::command]
pub async fn tweaks_apply(tweaks: State<'_, Arc<Tweaks>>, engine: State<'_, Engine>, changes: Vec<Change>) -> AppResult<HistoryEntry> {
    let t = tweaks.inner().clone();
    submit(&engine, move || t.apply(&changes)).await
}

#[tauri::command]
pub async fn system_info() -> AppResult<SystemInfo> {
    blocking(system::info).await
}

#[tauri::command]
pub async fn power_set_active(engine: State<'_, Engine>, guid: String) -> AppResult<HistoryEntry> {
    submit(&engine, move || system::set_active_plan(&guid)).await
}

#[tauri::command]
pub async fn power_add_plan(engine: State<'_, Engine>, which: String) -> AppResult<HistoryEntry> {
    submit(&engine, move || system::add_plan(&which)).await
}

#[tauri::command]
pub async fn power_delete_plan(engine: State<'_, Engine>, guid: String) -> AppResult<HistoryEntry> {
    submit(&engine, move || system::delete_plan(&guid)).await
}

#[tauri::command]
pub async fn power_restore_defaults(engine: State<'_, Engine>) -> AppResult<HistoryEntry> {
    engine.submit(system::restore_default_plans())
}

#[tauri::command]
pub async fn power_set_timeouts(engine: State<'_, Engine>, timeouts: Timeouts) -> AppResult<HistoryEntry> {
    submit(&engine, move || system::set_timeouts(timeouts)).await
}

#[tauri::command]
pub async fn power_set_hibernate(engine: State<'_, Engine>, on: bool) -> AppResult<HistoryEntry> {
    submit(&engine, move || system::set_hibernate(on)).await
}

#[tauri::command]
pub async fn pagefile_set(engine: State<'_, Engine>, automatic: bool, initial_mb: Option<u32>, maximum_mb: Option<u32>) -> AppResult<HistoryEntry> {
    submit(&engine, move || system::set_pagefile(automatic, initial_mb, maximum_mb)).await
}

#[tauri::command]
pub async fn cleanup_scan() -> AppResult<Vec<CleanupItem>> {
    blocking(system::cleanup_scan).await
}

#[tauri::command]
pub async fn cleanup_run(engine: State<'_, Engine>, ids: Vec<String>) -> AppResult<HistoryEntry> {
    submit(&engine, move || system::cleanup(&ids)).await
}

#[tauri::command]
pub async fn services_list(services: State<'_, Arc<Services>>) -> AppResult<Vec<ServiceInfo>> {
    let s = services.inner().clone();
    blocking(move || s.list()).await
}

#[tauri::command]
pub async fn services_set_start(services: State<'_, Arc<Services>>, engine: State<'_, Engine>, name: String, start: String) -> AppResult<HistoryEntry> {
    let start = match start.as_str() {
        "automatic" => StartType::Automatic,
        "manual" => StartType::Manual,
        "disabled" => StartType::Disabled,
        _ => return Err(AppError::Other("Invalid startup type.".into())),
    };
    let s = services.inner().clone();
    submit(&engine, move || s.set_start(&name, start)).await
}

#[tauri::command]
pub async fn services_control(services: State<'_, Arc<Services>>, engine: State<'_, Engine>, name: String, start: bool) -> AppResult<HistoryEntry> {
    let s = services.inner().clone();
    submit(&engine, move || s.control(&name, start)).await
}

#[tauri::command]
pub async fn startup_list(startup: State<'_, Arc<Startup>>) -> AppResult<StartupList> {
    let s = startup.inner().clone();
    blocking(move || s.list()).await
}

#[tauri::command]
pub async fn startup_set_enabled(startup: State<'_, Arc<Startup>>, engine: State<'_, Engine>, id: String, enabled: bool) -> AppResult<HistoryEntry> {
    let s = startup.inner().clone();
    submit(&engine, move || s.set_enabled(&id, enabled)).await
}

#[tauri::command]
pub async fn startup_remove(startup: State<'_, Arc<Startup>>, engine: State<'_, Engine>, id: String) -> AppResult<HistoryEntry> {
    let s = startup.inner().clone();
    submit(&engine, move || s.remove(&id)).await
}

#[tauri::command]
pub async fn startup_restore(startup: State<'_, Arc<Startup>>, engine: State<'_, Engine>, name: String, location: String) -> AppResult<HistoryEntry> {
    let s = startup.inner().clone();
    submit(&engine, move || s.restore(&name, &location)).await
}

#[tauri::command]
pub async fn processes_list(pm: State<'_, Arc<ProcessManager>>) -> AppResult<Vec<ProcessInfo>> {
    let pm = pm.inner().clone();
    blocking(move || Ok(pm.list())).await
}

#[tauri::command]
pub async fn processes_end(
    pm: State<'_, Arc<ProcessManager>>,
    engine: State<'_, Engine>,
    pid: u32,
    name: String,
    start_time: u64,
    tree: bool,
    as_admin: bool,
) -> AppResult<HistoryEntry> {
    let pm = pm.inner().clone();
    submit(&engine, move || pm.end(pid, &name, start_time, tree, as_admin)).await
}
