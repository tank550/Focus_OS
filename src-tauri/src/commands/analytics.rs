use tauri::State;

use crate::application::analytics::{self, AnalyticsSummary};
use crate::state::AppState;

#[tauri::command]
pub fn analytics_summary(state: State<'_, AppState>) -> Result<AnalyticsSummary, String> {
    analytics::summary(&state).map_err(|error| error.to_string())
}
