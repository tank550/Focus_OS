use tauri::State;

use focus_domain::autonomy::PolicyRule;

use crate::application::autonomy;
use crate::state::AppState;

/// Effective autonomy rule table (defaults + settings overrides), ordered.
#[tauri::command]
pub fn autonomy_policy(state: State<'_, AppState>) -> Result<Vec<PolicyRule>, String> {
    autonomy::rules(&state).map_err(|error| error.to_string())
}
