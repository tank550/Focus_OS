//! Read/classify surface over the deterministic autonomy policy.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use focus_domain::autonomy::{
    ActionContext, ActionType, AutonomyPolicy, AutonomyPolicyError, Classification, PolicyRule,
    RuleOverride,
};

use crate::application::settings;
use crate::infrastructure::db::DbError;
use crate::state::AppState;

/// JSON shape stored under `app_settings['autonomy']`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AutonomySettings {
    pub rules: Vec<RuleOverride>,
}

#[derive(Debug, Error)]
pub enum AutonomyError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error("invalid autonomy settings: {0}")]
    InvalidSettings(String),
    #[error(transparent)]
    Policy(#[from] AutonomyPolicyError),
}

/// Load the compiled-in rule table merged with the user's settings overrides.
pub fn policy(state: &AppState) -> Result<AutonomyPolicy, AutonomyError> {
    let overrides = settings::get_raw(state, settings::KEY_AUTONOMY)?
        .map(|raw| {
            serde_json::from_str::<AutonomySettings>(&raw)
                .map_err(|error| AutonomyError::InvalidSettings(error.to_string()))
        })
        .transpose()?
        .unwrap_or_default()
        .rules;
    Ok(AutonomyPolicy::with_overrides(&overrides)?)
}

/// Effective ordered rule table, for inspection in Settings.
pub fn rules(state: &AppState) -> Result<Vec<PolicyRule>, AutonomyError> {
    Ok(policy(state)?.rules().to_vec())
}

/// Deterministic classification of one proposed action.
pub fn classify(
    state: &AppState,
    action: ActionType,
    context: &ActionContext,
) -> Result<Classification, AutonomyError> {
    Ok(policy(state)?.classify(action, context))
}

#[cfg(test)]
mod tests {
    use super::*;
    use focus_domain::autonomy::AutonomyLevel;
    use focus_domain::ids::EntityId;

    fn test_state() -> (std::path::PathBuf, AppState) {
        let directory = std::env::temp_dir().join(format!("focus-os-autonomy-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        (directory, AppState::new(pool, database_path))
    }

    #[test]
    fn missing_settings_uses_compiled_defaults() {
        let (directory, state) = test_state();
        let classification = classify(
            &state,
            ActionType::MoveDeepWorkBlock,
            &ActionContext::default(),
        )
        .unwrap();
        assert_eq!(classification.level, AutonomyLevel::Propose);
        assert_eq!(classification.rule_id, "R10");
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn settings_overrides_are_applied_and_hard_rules_stay_confirmed() {
        let (directory, state) = test_state();
        settings::set_raw(
            &state,
            settings::KEY_AUTONOMY,
            r#"{"rules":[{"id":"R10","level":"AUTO_EXECUTE"},{"id":"R21","level":"AUTO_EXECUTE"}]}"#,
        )
        .unwrap();

        let moved = classify(
            &state,
            ActionType::MoveDeepWorkBlock,
            &ActionContext::default(),
        )
        .unwrap();
        assert_eq!(moved.level, AutonomyLevel::AutoExecute);

        let deleted =
            classify(&state, ActionType::DeleteProject, &ActionContext::default()).unwrap();
        assert_eq!(deleted.level, AutonomyLevel::RequireConfirmation);

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_settings_surface_as_an_error() {
        let (directory, state) = test_state();
        settings::set_raw(&state, settings::KEY_AUTONOMY, r#"{"rules":[{"id":"R99""#).unwrap();
        assert!(matches!(
            policy(&state),
            Err(AutonomyError::InvalidSettings(_))
        ));
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
