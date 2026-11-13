use tauri::State;

use crate::application::portfolio::{self, PortfolioReport};
use crate::infrastructure::github::GithubSourceSummary;
use crate::state::AppState;

#[tauri::command]
pub fn portfolio_assessments_list(
    state: State<'_, AppState>,
) -> Result<Vec<PortfolioReport>, String> {
    portfolio::list_recent(&state).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn portfolio_analyze_repository(
    state: State<'_, AppState>,
    full_name: String,
    confirmed_summary: GithubSourceSummary,
) -> Result<PortfolioReport, String> {
    portfolio::analyze_repository(&state, &full_name, &confirmed_summary)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn portfolio_save_draft(
    state: State<'_, AppState>,
    id: String,
    draft_markdown: String,
) -> Result<(), String> {
    portfolio::save_draft(&state, &id, &draft_markdown).map_err(|error| error.to_string())
}
