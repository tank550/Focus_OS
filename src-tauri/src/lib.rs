//! Focus OS — Rust core.
//!
//! Module layering (enforced by convention, verified by tests):
//!   commands → application → domain ← infrastructure (repositories)
//! The domain crate (`focus-domain`) is a pure library with no Tauri/SQLite
//! dependency; the webview has no SQL capability, so every mutation funnels
//! through the application services in this crate.

pub mod application;
pub mod commands;
pub mod infrastructure;
pub mod state;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .setup(|app| {
            // Window geometry persistence (Phase 1: remember window state).
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_window_state::Builder::default().build())?;

            // SQLite is the source of truth: open the app-data DB, apply
            // pending migrations, and publish the AppState to commands.
            let pool = infrastructure::db::init(app.handle())?;
            let db_path = app
                .path()
                .app_data_dir()?
                .join(infrastructure::db::DB_FILENAME);
            app.manage(state::AppState::new(pool, db_path));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::health,
            commands::goal_sidecar_add,
            commands::goal_sidecar_list,
            commands::goals_create,
            commands::goals_list,
            commands::inbox_archive,
            commands::inbox_capture,
            commands::inbox_list,
            commands::inbox_update,
            commands::projects_archive,
            commands::projects_list,
            commands::projects_update,
            commands::settings_get,
            commands::settings_set,
            commands::settings_all,
            commands::tasks_list_open,
            commands::tasks_current_focus,
            commands::tasks_start_focus,
            commands::tasks_pause_focus,
            commands::tasks_complete,
        ]);

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Second launch: focus the existing main window instead of
            // duplicating a process that could race on the SQLite file.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }

    builder
        .run(tauri::generate_context!())
        .expect("error while running Focus OS");
}
