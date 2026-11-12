use crate::infrastructure::secrets;

#[tauri::command]
pub fn ai_api_key_status() -> Result<bool, String> {
    secrets::has_nim_api_key().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn ai_api_key_store(api_key: String) -> Result<(), String> {
    secrets::store_nim_api_key(&api_key).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn ai_api_key_remove() -> Result<(), String> {
    secrets::remove_nim_api_key().map_err(|error| error.to_string())
}
