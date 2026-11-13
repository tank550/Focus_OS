use tauri::State;

use crate::application::agent;
use crate::application::inbox::{self, InboxItem};
use crate::infrastructure::ai::AiAnalysisResult;
use crate::state::AppState;

#[tauri::command]
pub fn inbox_capture(state: State<'_, AppState>, raw_text: String) -> Result<InboxItem, String> {
    inbox::capture(&state, &raw_text).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn inbox_list(state: State<'_, AppState>) -> Result<Vec<InboxItem>, String> {
    inbox::list(&state).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn inbox_update(
    state: State<'_, AppState>,
    id: String,
    raw_text: String,
) -> Result<(), String> {
    inbox::update(&state, &id, &raw_text).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn inbox_archive(state: State<'_, AppState>, id: String) -> Result<(), String> {
    inbox::archive(&state, &id).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn inbox_analyze(
    state: State<'_, AppState>,
    id: String,
) -> Result<AiAnalysisResult, String> {
    agent::analyze_inbox_note(&state, &id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn inbox_apply_task_proposal(
    state: State<'_, AppState>,
    id: String,
) -> Result<crate::application::tasks::Task, String> {
    agent::apply_task_proposal(&state, &id).map_err(|error| error.to_string())
}
