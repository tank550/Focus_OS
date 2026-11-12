use tauri::State;

use crate::application::projects::{self, Project};
use crate::state::AppState;

#[tauri::command]
pub fn projects_list(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    projects::list(&state).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn projects_update(
    state: State<'_, AppState>,
    id: String,
    name: String,
    description: Option<String>,
    status: String,
    priority: u16,
) -> Result<(), String> {
    projects::update(
        &state,
        &id,
        &name,
        description.as_deref(),
        &status,
        priority,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn projects_archive(state: State<'_, AppState>, id: String) -> Result<(), String> {
    projects::archive(&state, &id).map_err(|error| error.to_string())
}
