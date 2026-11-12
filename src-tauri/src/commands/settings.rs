use tauri::State;

use crate::application::settings;
use crate::state::AppState;

/// Read one settings key (JSON blob) or `null` when absent.
#[tauri::command]
pub fn settings_get(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    settings::get_raw(&state, &key).map_err(|e| e.to_string())
}

/// Upsert one settings key. Values are JSON-encoded blobs; secrets are never
/// stored here — only opaque keyring references (Phase 7 wires the keyring).
#[tauri::command]
pub fn settings_set(state: State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    settings::set_raw(&state, &key, &value).map_err(|e| e.to_string())
}

/// Full settings map (key -> JSON blob). Serializes to a JSON object.
#[tauri::command]
pub fn settings_all(
    state: State<'_, AppState>,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    settings::all_raw(&state).map_err(|e| e.to_string())
}
