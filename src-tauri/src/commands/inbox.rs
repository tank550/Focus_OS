use tauri::State;

use crate::application::inbox::{self, InboxItem};
use crate::state::AppState;

#[tauri::command]
pub fn inbox_capture(state: State<'_, AppState>, raw_text: String) -> Result<InboxItem, String> {
    inbox::capture(&state, &raw_text).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn inbox_list(state: State<'_, AppState>) -> Result<Vec<InboxItem>, String> {
    inbox::list(&state).map_err(|error| error.to_string())
}
