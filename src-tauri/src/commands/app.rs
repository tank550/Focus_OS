use tauri::State;

use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub tauri_version: String,
    /// Minimum supported Rust version, from Cargo.toml `rust-version`.
    pub msrv: String,
}

#[tauri::command]
pub fn app_info(_state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        name: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        tauri_version: tauri::VERSION.to_string(),
        msrv: env!("CARGO_PKG_RUST_VERSION").to_string(),
    }
}