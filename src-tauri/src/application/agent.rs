use serde::{Deserialize, Serialize};
use thiserror::Error;

use focus_domain::ids::EntityId;
use focus_domain::value_objects::UtcMillis;

use crate::application::inbox::InboxError;
use crate::application::settings::{self, AiSettings};
use crate::application::tasks::{self, AgentTaskDraft, Task, TasksError};
use crate::infrastructure::ai::{
    Actionability, AiAnalysisResult, AiProviderError, AiProviderKind, AnalysisKind,
    BrainDumpAnalysis, MockProvider, NimProvider,
};
use crate::infrastructure::db::DbError;
use crate::infrastructure::repositories::autonomy_repo::{AutonomyDecisionRecord, AutonomyRepo};
use crate::infrastructure::repositories::inbox_repo::{AnalysisProposal, InboxRepo};
use crate::infrastructure::repositories::tasks_repo::TasksRepo;
use crate::infrastructure::secrets::{self, SecretError};
use crate::state::AppState;

const NIM_DEFAULT_BASE_URL: &str = "https://integrate.api.nvidia.com";

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("AI analysis is disabled in Settings")]
    Disabled,
    #[error("The inbox note was not found or is archived")]
    NoteNotFound,
    #[error("The note changed while it was being analyzed; run the analysis again")]
    NoteChanged,
    #[error("Choose an NIM model in Settings before analyzing a note")]
    MissingModel,
    #[error("Save an NVIDIA NIM API key in Settings before analyzing a note")]
    MissingApiKey,
    #[error("Unsupported AI provider: {0}")]
    UnsupportedProvider(String),
    #[error("Could not read AI settings: {0}")]
    InvalidSettings(String),
    #[error("Could not store the analysis: {0}")]
    Serialization(String),
    #[error("This note has no pending proposal to apply")]
    ProposalNotFound,
    #[error("Only actionable task proposals with a next step can create a task")]
    ProposalNotApplicable,
    #[error("The previously created task for this proposal could not be found")]
    AppliedTaskNotFound,
    #[error(transparent)]
    Provider(#[from] AiProviderError),
    #[error(transparent)]
    Secrets(#[from] SecretError),
    #[error(transparent)]
    Inbox(#[from] InboxError),
    #[error(transparent)]
    Task(#[from] TasksError),
    #[error(transparent)]
    Database(#[from] DbError),
}

#[derive(Debug, Deserialize, Serialize)]
struct StoredAnalysis {
    provider: AiProviderKind,
    analysis: BrainDumpAnalysis,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskActionResult {
    task_id: String,
}

pub async fn analyze_inbox_note(
    state: &AppState,
    inbox_item_id: &str,
) -> Result<AiAnalysisResult, AgentError> {
    let record = {
        let conn = state.conn()?;
        InboxRepo::get(&conn, inbox_item_id)?.ok_or(AgentError::NoteNotFound)?
    };

    let ai_settings = settings::get_raw(state, settings::KEY_AI)?
        .map(|raw| {
            serde_json::from_str::<AiSettings>(&raw)
                .map_err(|error| AgentError::InvalidSettings(error.to_string()))
        })
        .transpose()?
        .unwrap_or_default();
    if !ai_settings.enabled {
        return Err(AgentError::Disabled);
    }

    let result = match ai_settings.provider.as_str() {
        "mock" => AiAnalysisResult {
            provider: AiProviderKind::Mock,
            analysis: MockProvider::analyze(),
        },
        "nim" => {
            if ai_settings
                .model
                .as_deref()
                .map(str::trim)
                .filter(|model| !model.is_empty())
                .is_none()
            {
                return Err(AgentError::MissingModel);
            }
            if !secrets::has_nim_api_key()? {
                return Err(AgentError::MissingApiKey);
            }
            let api_key = secrets::load_nim_api_key()?;
            let provider = NimProvider::new(
                ai_settings
                    .base_url
                    .as_deref()
                    .filter(|url| !url.trim().is_empty())
                    .unwrap_or(NIM_DEFAULT_BASE_URL),
                ai_settings
                    .model
                    .as_deref()
                    .ok_or(AgentError::MissingModel)?,
                api_key,
            )?;
            AiAnalysisResult {
                provider: AiProviderKind::Nim,
                analysis: provider.analyze(&record.raw_text).await?,
            }
        }
        provider => return Err(AgentError::UnsupportedProvider(provider.to_owned())),
    };

    let serialized = serde_json::to_string(&StoredAnalysis {
        provider: result.provider,
        analysis: result.analysis.clone(),
    })
    .map_err(|error| AgentError::Serialization(error.to_string()))?;
    let analyzed_at = UtcMillis::now().0;
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    let proposal_id = EntityId::new().to_string();
    if !InboxRepo::record_analysis(
        &tx,
        &AnalysisProposal {
            inbox_item_id,
            expected_text: &record.raw_text,
            expected_updated_at: record.updated_at,
            proposal_id: &proposal_id,
            proposal_json: &serialized,
            kind: result.analysis.kind.as_str(),
            confidence: result.analysis.confidence,
            analyzed_at,
        },
    )? {
        return Err(AgentError::NoteChanged);
    }
    tx.commit().map_err(DbError::from)?;
    Ok(result)
}

pub fn apply_task_proposal(state: &AppState, inbox_item_id: &str) -> Result<Task, AgentError> {
    let mut conn = state.conn()?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(DbError::from)?;

    let Some(proposal) = InboxRepo::pending_proposal(&tx, inbox_item_id)? else {
        if let Some(result_payload) = InboxRepo::applied_task_action(&tx, inbox_item_id)? {
            let result: TaskActionResult = serde_json::from_str(&result_payload)
                .map_err(|error| AgentError::Serialization(error.to_string()))?;
            let record =
                TasksRepo::get(&tx, &result.task_id)?.ok_or(AgentError::AppliedTaskNotFound)?;
            return Ok(Task::try_from(record)?);
        }
        return Err(AgentError::ProposalNotFound);
    };

    let stored: StoredAnalysis = serde_json::from_str(&proposal.proposal)
        .map_err(|error| AgentError::Serialization(error.to_string()))?;
    if stored.provider != AiProviderKind::Nim {
        return Err(AgentError::ProposalNotApplicable);
    }
    let analysis = stored.analysis;
    if analysis.kind != AnalysisKind::Task || analysis.actionability != Actionability::Actionable {
        return Err(AgentError::ProposalNotApplicable);
    }
    let title = analysis
        .next_step
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(AgentError::ProposalNotApplicable)?;
    if analysis.estimated_duration_minutes == 0 || analysis.priority > 100 {
        return Err(AgentError::ProposalNotApplicable);
    }

    let task = tasks::create_from_agent_in_transaction(
        &tx,
        AgentTaskDraft {
            title: title.to_owned(),
            description: Some(format!("Created from Inbox note {inbox_item_id}")),
            project_id: None,
            goal_id: None,
            estimated_minutes: analysis.estimated_duration_minutes,
            priority: u16::from(analysis.priority),
            requires_deep_work: false,
        },
    )?;
    let now = UtcMillis::now().0;
    if !InboxRepo::approve_proposal(&tx, inbox_item_id, &proposal.id, now)? {
        return Err(AgentError::ProposalNotFound);
    }

    let policy = serde_json::json!({
        "decision": "explicit_user_confirmation",
        "scope": "create_task_from_actionable_task_proposal",
    })
    .to_string();
    let result_payload = serde_json::to_string(&TaskActionResult {
        task_id: task.id.clone(),
    })
    .map_err(|error| AgentError::Serialization(error.to_string()))?;
    let related_task_ids = serde_json::to_string(std::slice::from_ref(&task.id))
        .map_err(|error| AgentError::Serialization(error.to_string()))?;
    let decision_id = EntityId::new().to_string();
    AutonomyRepo::insert_confirmed_task_from_inbox(
        &tx,
        &AutonomyDecisionRecord {
            id: &decision_id,
            action_id: &proposal.id,
            occurred_at: now,
            policy: &policy,
            reason: "The user explicitly approved creating this task from the Inbox proposal.",
            proposed_payload: &proposal.proposal,
            result_payload: &result_payload,
            related_task_ids: &related_task_ids,
        },
    )?;

    tx.commit().map_err(DbError::from)?;
    Ok(task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::inbox;
    use crate::infrastructure::ai::{Actionability, AnalysisKind, BrainDumpAnalysis};
    use crate::infrastructure::repositories::inbox_repo::{AnalysisProposal, InboxRepo};

    fn test_state() -> (std::path::PathBuf, AppState) {
        let directory = std::env::temp_dir().join(format!("focus-os-agent-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        (directory, AppState::new(pool, database_path))
    }

    fn add_task_proposal(state: &AppState, inbox_item_id: &str, provider: AiProviderKind) {
        let conn = state.conn().unwrap();
        let note = InboxRepo::get(&conn, inbox_item_id)
            .unwrap()
            .expect("captured note exists");
        let serialized = serde_json::to_string(&StoredAnalysis {
            provider,
            analysis: BrainDumpAnalysis {
                kind: AnalysisKind::Task,
                project_guess: None,
                goal_guess: None,
                estimated_duration_minutes: 25,
                priority: 60,
                deadline: None,
                actionability: Actionability::Actionable,
                next_step: Some("Draft the first section".into()),
                dependencies: None,
                confidence: 0.9,
            },
        })
        .unwrap();
        let mut conn = conn;
        let tx = conn.transaction().unwrap();
        let proposal_id = EntityId::new().to_string();
        assert!(InboxRepo::record_analysis(
            &tx,
            &AnalysisProposal {
                inbox_item_id,
                expected_text: &note.raw_text,
                expected_updated_at: note.updated_at,
                proposal_id: &proposal_id,
                proposal_json: &serialized,
                kind: "TASK",
                confidence: 0.9,
                analyzed_at: UtcMillis::now().0,
            }
        )
        .unwrap());
        tx.commit().unwrap();
    }

    #[tokio::test]
    async fn mock_analysis_is_saved_as_a_proposal_without_mutating_the_note() {
        let (directory, state) = test_state();
        settings::set_raw(
            &state,
            settings::KEY_AI,
            r#"{"enabled":true,"provider":"mock","baseUrl":null,"model":null,"apiKeyStored":false}"#,
        )
        .unwrap();
        let note = inbox::capture(&state, "A thought to sort later").unwrap();

        let result = analyze_inbox_note(&state, &note.id).await.unwrap();

        assert_eq!(result.provider, AiProviderKind::Mock);
        assert_eq!(result.analysis.actionability, Actionability::NotActionable);
        let updated_note = inbox::list(&state).unwrap().remove(0);
        assert_eq!(updated_note.raw_text, note.raw_text);
        assert_eq!(updated_note.kind, "NOTE");
        assert_eq!(updated_note.status, "PROPOSED");
        assert_eq!(updated_note.analysis, Some(result));

        let conn = state.conn().unwrap();
        let proposal_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM classification_proposals
                 WHERE inbox_item_id = ?1 AND status = 'PENDING'",
                [&note.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(proposal_count, 1);
        drop(conn);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn confirmed_actionable_task_proposal_is_created_once_and_audited() {
        let (directory, state) = test_state();
        let note = inbox::capture(&state, "I need to write the report").unwrap();
        add_task_proposal(&state, &note.id, AiProviderKind::Nim);

        let task = apply_task_proposal(&state, &note.id).unwrap();
        assert_eq!(task.title, "Draft the first section");
        assert_eq!(task.estimated_minutes, 25);
        assert_eq!(task.priority, 60);
        assert_eq!(
            task.description.as_deref(),
            Some(format!("Created from Inbox note {}", note.id).as_str())
        );

        let retried_task = apply_task_proposal(&state, &note.id).unwrap();
        assert_eq!(retried_task.id, task.id);

        let updated_note = inbox::list(&state).unwrap().remove(0);
        assert_eq!(updated_note.raw_text, note.raw_text);
        assert_eq!(updated_note.status, "CONVERTED");

        let conn = state.conn().unwrap();
        let (task_count, audit_count, proposal_status, audit_status): (i64, i64, String, String) =
            conn.query_row(
                "SELECT
                    (SELECT COUNT(*) FROM tasks WHERE id = ?2),
                    (SELECT COUNT(*) FROM autonomy_decisions
                     WHERE action_type = 'CREATE_TASK_FROM_INBOX'),
                    (SELECT status FROM classification_proposals WHERE inbox_item_id = ?1),
                    (SELECT status FROM autonomy_decisions
                     WHERE action_type = 'CREATE_TASK_FROM_INBOX')",
                rusqlite::params![note.id, task.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(task_count, 1);
        assert_eq!(audit_count, 1);
        assert_eq!(proposal_status, "APPROVED");
        assert_eq!(audit_status, "CONFIRMED");

        drop(conn);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn mock_proposals_cannot_create_tasks_even_if_marked_actionable() {
        let (directory, state) = test_state();
        let note = inbox::capture(&state, "A thought").unwrap();
        add_task_proposal(&state, &note.id, AiProviderKind::Mock);

        assert!(matches!(
            apply_task_proposal(&state, &note.id),
            Err(AgentError::ProposalNotApplicable)
        ));
        assert!(crate::application::tasks::list_open(&state)
            .unwrap()
            .is_empty());

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn task_creation_and_proposal_approval_roll_back_when_audit_insert_fails() {
        let (directory, state) = test_state();
        let note = inbox::capture(&state, "I need to write the report").unwrap();
        add_task_proposal(&state, &note.id, AiProviderKind::Nim);
        {
            let conn = state.conn().unwrap();
            conn.execute_batch(
                "CREATE TRIGGER reject_test_autonomy_decision
                 BEFORE INSERT ON autonomy_decisions
                 BEGIN
                     SELECT RAISE(ABORT, 'test audit failure');
                 END;",
            )
            .unwrap();
        }

        assert!(matches!(
            apply_task_proposal(&state, &note.id),
            Err(AgentError::Database(_))
        ));
        let conn = state.conn().unwrap();
        let (task_count, proposal_status, inbox_status): (i64, String, String) = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM tasks),
                    (SELECT status FROM classification_proposals WHERE inbox_item_id = ?1),
                    (SELECT status FROM inbox_items WHERE id = ?1)",
                [&note.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(task_count, 0);
        assert_eq!(proposal_status, "PENDING");
        assert_eq!(inbox_status, "PROPOSED");

        drop(conn);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
