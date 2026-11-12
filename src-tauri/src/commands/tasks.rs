use tauri::State;

use crate::application::tasks::{self, FocusSession, Task};
use crate::state::AppState;

#[tauri::command]
pub fn tasks_list_open(state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    tasks::list_open(&state).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn tasks_current_focus(state: State<'_, AppState>) -> Result<Option<FocusSession>, String> {
    tasks::current_focus(&state).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn tasks_start_focus(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<FocusSession, String> {
    tasks::start_focus(&state, &task_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn tasks_pause_focus(state: State<'_, AppState>, task_id: String) -> Result<(), String> {
    tasks::pause_focus(&state, &task_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn tasks_complete(state: State<'_, AppState>, task_id: String) -> Result<(), String> {
    tasks::complete(&state, &task_id).map_err(|error| error.to_string())
}
