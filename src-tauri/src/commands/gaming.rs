use std::sync::Arc;

use tauri::State;

use crate::engine::types::{HistoryEntry, JobRequest};
use crate::engine::Engine;
use crate::error::{AppError, AppResult};
use crate::gaming::display::{self, Display};
use crate::gaming::fps::{FpsMonitor, FpsSample};
use crate::gaming::games::{self, ExeCandidate, Game, Games, Profile};
use crate::gaming::{Gaming, LastPreset, PresetInfo};

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::Other(e.to_string()))?
}

async fn submit(engine: &Engine, build: impl FnOnce() -> AppResult<JobRequest> + Send + 'static) -> AppResult<HistoryEntry> {
    let req = blocking(build).await?;
    engine.submit(req)
}

#[tauri::command]
pub async fn gaming_presets(gaming: State<'_, Arc<Gaming>>) -> AppResult<(Vec<PresetInfo>, Option<LastPreset>)> {
    let g = gaming.inner().clone();
    blocking(move || Ok((g.presets()?, g.last()))).await
}

#[tauri::command]
pub async fn gaming_apply_preset(gaming: State<'_, Arc<Gaming>>, engine: State<'_, Engine>, id: String) -> AppResult<HistoryEntry> {
    let g = gaming.inner().clone();
    submit(&engine, move || g.apply_preset(&id)).await
}

#[tauri::command]
pub async fn gaming_revert_preset(gaming: State<'_, Arc<Gaming>>, engine: State<'_, Engine>) -> AppResult<HistoryEntry> {
    let g = gaming.inner().clone();
    submit(&engine, move || g.revert_last()).await
}

#[tauri::command]
pub async fn games_list(games: State<'_, Arc<Games>>) -> AppResult<Vec<(Game, Option<Profile>)>> {
    let g = games.inner().clone();
    blocking(move || Ok(g.list()?.into_iter().map(|game| {
        let profile = game.exe.as_deref().map(games::read_profile);
        (game, profile)
    }).collect())).await
}

#[tauri::command]
pub async fn games_add(games: State<'_, Arc<Games>>, name: String, exe: String) -> AppResult<Game> {
    let g = games.inner().clone();
    blocking(move || g.add_custom(&name, &exe)).await
}

#[tauri::command]
pub async fn games_remove(games: State<'_, Arc<Games>>, id: String) -> AppResult<()> {
    games.remove_custom(&id);
    Ok(())
}

#[tauri::command]
pub async fn games_candidates(games: State<'_, Arc<Games>>, id: String) -> AppResult<Vec<ExeCandidate>> {
    let g = games.inner().clone();
    blocking(move || g.candidates(&id)).await
}

#[tauri::command]
pub async fn games_choose_exe(games: State<'_, Arc<Games>>, id: String, exe: String) -> AppResult<()> {
    let g = games.inner().clone();
    blocking(move || g.choose_exe(&id, &exe)).await
}

#[tauri::command]
pub async fn games_set_profile(games: State<'_, Arc<Games>>, engine: State<'_, Engine>, exe: String, profile: Profile) -> AppResult<HistoryEntry> {
    let g = games.inner().clone();
    submit(&engine, move || g.set_profile(&exe, profile)).await
}

#[tauri::command]
pub async fn displays_list() -> AppResult<Vec<Display>> {
    blocking(display::list).await
}

#[tauri::command]
pub async fn display_set_refresh_rate(engine: State<'_, Engine>, device: String, hz: u32) -> AppResult<HistoryEntry> {
    submit(&engine, move || display::set_refresh_rate(&device, hz)).await
}

#[tauri::command]
pub async fn fps_start(fps: State<'_, Arc<FpsMonitor>>, engine: State<'_, Engine>, elevate: bool) -> AppResult<()> {
    let f = fps.inner().clone();
    let elevated = engine.is_elevated();
    blocking(move || f.start(elevated, elevate)).await
}

#[tauri::command]
pub async fn fps_poll(fps: State<'_, Arc<FpsMonitor>>) -> AppResult<(bool, Option<FpsSample>)> {
    Ok(fps.poll())
}

#[tauri::command]
pub async fn fps_stop(fps: State<'_, Arc<FpsMonitor>>) -> AppResult<()> {
    fps.stop();
    Ok(())
}
