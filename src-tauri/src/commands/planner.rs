use tauri::State;

use crate::application::planner::{self, DailyPlan};
use crate::state::AppState;

#[tauri::command]
pub fn planner_today(
    state: State<'_, AppState>,
    plan_date: String,
) -> Result<Option<DailyPlan>, String> {
    planner::today(&state, &plan_date).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn planner_plan_day(
    state: State<'_, AppState>,
    plan_date: String,
) -> Result<DailyPlan, String> {
    planner::plan_day(&state, &plan_date).map_err(|error| error.to_string())
}
