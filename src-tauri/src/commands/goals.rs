use tauri::State;

use crate::application::goals::{self, Goal, SidecarEntry};
use crate::state::AppState;

#[tauri::command]
pub fn goals_list(state: State<'_, AppState>) -> Result<Vec<Goal>, String> {
    goals::list(&state).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn goals_create(
    state: State<'_, AppState>,
    title: String,
    description: Option<String>,
    expected_outcome: Option<String>,
    success_criteria: Vec<String>,
) -> Result<Goal, String> {
    goals::create(
        &state,
        &title,
        description.as_deref(),
        expected_outcome.as_deref(),
        &success_criteria,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn goal_sidecar_list(
    state: State<'_, AppState>,
    goal_id: String,
) -> Result<Vec<SidecarEntry>, String> {
    goals::list_sidecar(&state, &goal_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn goal_sidecar_add(
    state: State<'_, AppState>,
    goal_id: String,
    content: String,
) -> Result<SidecarEntry, String> {
    goals::add_check_in(&state, &goal_id, &content).map_err(|error| error.to_string())
}
