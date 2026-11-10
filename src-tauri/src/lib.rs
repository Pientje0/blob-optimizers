mod apps;
mod backup;
mod commands;
mod engine;
mod error;
mod gaming;
mod network;
mod settings;
mod state;
mod system;
mod tweaks;
mod updates;
mod util;

use std::sync::Arc;

use tauri::{Emitter, Manager};

/// Runs the elevated FPS collector (see `gaming::fps`); used by `main` for `--fps-helper`.
pub fn fps_helper(args: &[String]) -> i32 {
    #[cfg(windows)]
    {
        gaming::fps::run_helper(args)
    }
    #[cfg(not(windows))]
    {
        let _ = args;
        1
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(state::AppState::new())
        .manage(Arc::new(system::processes::ProcessManager::new()))
        .setup(|app| {
            let base = app.path().app_local_data_dir()?;
            app.manage(Arc::new(apps::Apps::new(base.join("apps"))));
            let tweaks = Arc::new(tweaks::Tweaks::new(base.join("tweaks")));
            app.manage(tweaks.clone());
            app.manage(Arc::new(gaming::Gaming::new(tweaks, base.join("gaming"))));
            app.manage(Arc::new(gaming::games::Games::new(base.join("gaming"))));
            app.manage(Arc::new(gaming::fps::FpsMonitor::new(base.join("gaming").join("fps"))));
            app.manage(Arc::new(tweaks::services::Services::new(base.join("tweaks"))));
            app.manage(Arc::new(tweaks::startup::Startup::new(base.join("tweaks"))));
            let handle = app.handle().clone();
            let engine = engine::Engine::new(
                base.join("engine"),
                util::elevation::is_elevated(),
                Box::new(move |event, payload| {
                    let _ = handle.emit(event, payload);
                }),
            );
            let settings = Arc::new(settings::SettingsStore::load(base.clone()));
            let prefs = settings.get();
            engine.set_history_limit(prefs.history_limit);
            app.manage(engine);
            app.manage(settings);

            // Launched at sign-in with "start minimized" chosen in Settings.
            if prefs.start_minimized && std::env::args().any(|a| a == "--minimized-if-set") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::system::get_live_stats,
            commands::system::get_hardware_info,
            commands::engine::engine_catalog,
            commands::engine::engine_run_operation,
            commands::engine::engine_validate,
            commands::engine::engine_run_command,
            commands::engine::engine_cancel,
            commands::engine::engine_active,
            commands::engine::engine_output,
            commands::engine::engine_history,
            commands::engine::engine_clear_history,
            commands::engine::relaunch_as_admin,
            commands::apps::apps_inventory,
            commands::apps::apps_uninstall,
            commands::apps::apps_remove_store,
            commands::apps::apps_removed,
            commands::apps::apps_reinstall,
            commands::apps::apps_scan_leftovers,
            commands::apps::apps_cleanup_leftovers,
            commands::apps::features_list,
            commands::apps::features_toggle,
            commands::tweaks::tweaks_list,
            commands::tweaks::tweaks_apply,
            commands::tweaks::system_info,
            commands::tweaks::power_set_active,
            commands::tweaks::power_add_plan,
            commands::tweaks::power_delete_plan,
            commands::tweaks::power_restore_defaults,
            commands::tweaks::power_set_timeouts,
            commands::tweaks::power_set_hibernate,
            commands::tweaks::pagefile_set,
            commands::tweaks::cleanup_scan,
            commands::tweaks::cleanup_run,
            commands::tweaks::services_list,
            commands::tweaks::services_set_start,
            commands::tweaks::services_control,
            commands::tweaks::startup_list,
            commands::tweaks::startup_set_enabled,
            commands::tweaks::startup_remove,
            commands::tweaks::startup_restore,
            commands::tweaks::processes_list,
            commands::tweaks::processes_end,
            commands::gaming::gaming_presets,
            commands::gaming::gaming_apply_preset,
            commands::gaming::gaming_revert_preset,
            commands::gaming::games_list,
            commands::gaming::games_add,
            commands::gaming::games_remove,
            commands::gaming::games_candidates,
            commands::gaming::games_choose_exe,
            commands::gaming::games_set_profile,
            commands::gaming::displays_list,
            commands::gaming::display_set_refresh_rate,
            commands::gaming::fps_start,
            commands::gaming::fps_poll,
            commands::gaming::fps_stop,
            commands::advanced::network_adapters,
            commands::advanced::network_set_dns,
            commands::advanced::network_set_adapter,
            commands::advanced::network_set_category,
            commands::advanced::network_diagnose,
            commands::advanced::network_dns_presets,
            commands::advanced::network_dns_benchmark,
            commands::advanced::restore_status,
            commands::advanced::restore_list,
            commands::advanced::restore_rollback,
            commands::advanced::backups_registry,
            commands::advanced::backups_registry_import,
            commands::advanced::profile_export,
            commands::advanced::profile_list,
            commands::advanced::profile_preview,
            commands::advanced::settings_get,
            commands::advanced::updates_check,
            commands::advanced::updates_open,
            commands::advanced::settings_set,
            commands::advanced::app_paths,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Blob Optimizers");
}
