use serde::{Deserialize, Serialize};
use thiserror::Error;

use focus_domain::ids::EntityId;
use focus_domain::value_objects::UtcMillis;

use crate::application::inbox::InboxError;
use crate::application::projects::{self, AgentProjectDraft, Project, ProjectsError};
use crate::application::settings::{self, AiSettings};
use crate::application::tasks::{self, AgentTaskDraft, Task, TasksError};
use crate::infrastructure::ai::{
    Actionability, AiAnalysisResult, AiProviderError, AiProviderKind, AnalysisKind,
    BrainDumpAnalysis, MockProvider, NimProvider,
};
use crate::infrastructure::db::DbError;
use crate::infrastructure::repositories::autonomy_repo::{
    AutonomyDecisionRecord, AutonomyRepo, ConfirmedInboxAction,
};
use crate::infrastructure::repositories::inbox_repo::{AnalysisProposal, InboxRepo};
use crate::infrastructure::repositories::projects_repo::ProjectsRepo;
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
    #[error("The previously created task for this proposal could not be found")]
    AppliedTaskNotFound,
    #[error("The previously created project for this proposal could not be found")]
    AppliedProjectNotFound,
    #[error("Only actionable task or project proposals can be applied")]
    UnsupportedOperation,
    #[error("The project proposal does not include a project name")]
    MissingProjectName,
    #[error(transparent)]
    Provider(#[from] AiProviderError),
    #[error(transparent)]
    Secrets(#[from] SecretError),
    #[error(transparent)]
    Inbox(#[from] InboxError),
    #[error(transparent)]
    Task(#[from] TasksError),
    #[error(transparent)]
    Project(#[from] ProjectsError),
    #[error(transparent)]
    Database(#[from] DbError),
}

#[derive(Debug, Deserialize, Serialize)]
struct StoredAnalysis {
    provider: AiProviderKind,
    analysis: BrainDumpAnalysis,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppliedProposalPayload {
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    task_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedInboxProposal {
    pub project: Option<Project>,
    pub task: Option<Task>,
}

enum ProposedOperation {
    CreateTask(AgentTaskDraft),
    CreateProject {
        project: AgentProjectDraft,
        first_task: Option<AgentTaskDraft>,
    },
}

impl ProposedOperation {
    fn action_type(&self) -> ConfirmedInboxAction {
        match self {
            Self::CreateTask(_) => ConfirmedInboxAction::CreateTask,
            Self::CreateProject { .. } => ConfirmedInboxAction::CreateProject,
        }
    }
}

fn validated_operation(
    provider: AiProviderKind,
    analysis: BrainDumpAnalysis,
    raw_text: &str,
    inbox_item_id: &str,
) -> Result<ProposedOperation, AgentError> {
    if provider != AiProviderKind::Nim || analysis.actionability != Actionability::Actionable {
        return Err(AgentError::UnsupportedOperation);
    }
    if analysis.estimated_duration_minutes == 0 || analysis.priority > 100 {
        return Err(AgentError::UnsupportedOperation);
    }
    let next_step = analysis
        .next_step
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);

    match analysis.kind {
        AnalysisKind::Task => {
            let title = next_step.ok_or(AgentError::UnsupportedOperation)?;
            Ok(ProposedOperation::CreateTask(AgentTaskDraft {
                title,
                description: Some(format!("Created from Inbox note {inbox_item_id}")),
                project_id: None,
                goal_id: None,
                estimated_minutes: analysis.estimated_duration_minutes,
                priority: u16::from(analysis.priority),
                requires_deep_work: false,
            }))
        }
        AnalysisKind::Project => {
            let name = analysis
                .project_guess
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or(AgentError::MissingProjectName)?;
            let first_task = next_step.map(|title| AgentTaskDraft {
                title,
                description: Some(format!("First step from Inbox note {inbox_item_id}")),
                project_id: None,
                goal_id: None,
                estimated_minutes: analysis.estimated_duration_minutes,
                priority: u16::from(analysis.priority),
                requires_deep_work: false,
            });
            Ok(ProposedOperation::CreateProject {
                project: AgentProjectDraft {
                    name,
                    description: Some(raw_text.trim().to_owned()),
                    priority: u16::from(analysis.priority),
                },
                first_task,
            })
        }
        _ => Err(AgentError::UnsupportedOperation),
    }
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

pub fn apply_proposal(
    state: &AppState,
    inbox_item_id: &str,
) -> Result<AppliedInboxProposal, AgentError> {
    let mut conn = state.conn()?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(DbError::from)?;

    let Some(proposal) = InboxRepo::pending_proposal(&tx, inbox_item_id)? else {
        if let Some(result_payload) = InboxRepo::applied_proposal_action(&tx, inbox_item_id)? {
            let result: AppliedProposalPayload = serde_json::from_str(&result_payload)
                .map_err(|error| AgentError::Serialization(error.to_string()))?;
            let project = result
                .project_id
                .map(|project_id| {
                    ProjectsRepo::get(&tx, &project_id)?
                        .map(Project::from)
                        .ok_or(AgentError::AppliedProjectNotFound)
                })
                .transpose()?;
            let task = result
                .task_id
                .map(|task_id| {
                    TasksRepo::get(&tx, &task_id)?
                        .map(Task::try_from)
                        .transpose()?
                        .ok_or(AgentError::AppliedTaskNotFound)
                })
                .transpose()?;
            return Ok(AppliedInboxProposal { project, task });
        }
        return Err(AgentError::ProposalNotFound);
    };

    let stored: StoredAnalysis = serde_json::from_str(&proposal.proposal)
        .map_err(|error| AgentError::Serialization(error.to_string()))?;
    let operation = validated_operation(
        stored.provider,
        stored.analysis,
        &proposal.raw_text,
        inbox_item_id,
    )?;
    let action_type = operation.action_type();
    let (project, task) = match operation {
        ProposedOperation::CreateTask(draft) => (
            None,
            Some(tasks::create_from_agent_in_transaction(&tx, draft)?),
        ),
        ProposedOperation::CreateProject {
            project: project_draft,
            first_task,
        } => {
            let project = projects::create_from_agent_in_transaction(&tx, project_draft)?;
            let task = first_task
                .map(|mut draft| {
                    draft.project_id = Some(project.id.clone());
                    tasks::create_from_agent_in_transaction(&tx, draft)
                })
                .transpose()?;
            (Some(project), task)
        }
    };
    let now = UtcMillis::now().0;
    if !InboxRepo::approve_proposal(&tx, inbox_item_id, &proposal.id, now)? {
        return Err(AgentError::ProposalNotFound);
    }

    let policy = serde_json::json!({
        "decision": "explicit_user_confirmation",
        "harness": "validated_closed_operation_v1",
        "scope": action_type.as_str(),
    })
    .to_string();
    let result_payload = serde_json::to_string(&AppliedProposalPayload {
        project_id: project.as_ref().map(|value| value.id.clone()),
        task_id: task.as_ref().map(|value| value.id.clone()),
    })
    .map_err(|error| AgentError::Serialization(error.to_string()))?;
    let related_task_ids = serde_json::to_string(
        &task
            .as_ref()
            .map(|value| vec![value.id.clone()])
            .unwrap_or_default(),
    )
    .map_err(|error| AgentError::Serialization(error.to_string()))?;
    let decision_id = EntityId::new().to_string();
    AutonomyRepo::insert_confirmed_inbox_action(
        &tx,
        action_type,
        &AutonomyDecisionRecord {
            id: &decision_id,
            action_id: &proposal.id,
            occurred_at: now,
            policy: &policy,
            reason: match action_type {
                ConfirmedInboxAction::CreateTask => {
                    "The user explicitly approved creating this task from the Inbox proposal."
                }
                ConfirmedInboxAction::CreateProject => {
                    "The user explicitly approved creating this project from the Inbox proposal."
                }
            },
            proposed_payload: &proposal.proposal,
            result_payload: &result_payload,
            related_task_ids: &related_task_ids,
        },
    )?;

    tx.commit().map_err(DbError::from)?;
    Ok(AppliedInboxProposal { project, task })
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

    fn add_proposal(
        state: &AppState,
        inbox_item_id: &str,
        provider: AiProviderKind,
        analysis: BrainDumpAnalysis,
    ) {
        let conn = state.conn().unwrap();
        let note = InboxRepo::get(&conn, inbox_item_id)
            .unwrap()
            .expect("captured note exists");
        let serialized = serde_json::to_string(&StoredAnalysis {
            provider,
            analysis: analysis.clone(),
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
                kind: analysis.kind.as_str(),
                confidence: analysis.confidence,
                analyzed_at: UtcMillis::now().0,
            }
        )
        .unwrap());
        tx.commit().unwrap();
    }

    fn add_task_proposal(state: &AppState, inbox_item_id: &str, provider: AiProviderKind) {
        add_proposal(
            state,
            inbox_item_id,
            provider,
            BrainDumpAnalysis {
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
        );
    }

    fn add_project_proposal(state: &AppState, inbox_item_id: &str, next_step: Option<&str>) {
        add_proposal(
            state,
            inbox_item_id,
            AiProviderKind::Nim,
            BrainDumpAnalysis {
                kind: AnalysisKind::Project,
                project_guess: Some("Launch the newsletter".into()),
                goal_guess: None,
                estimated_duration_minutes: 30,
                priority: 70,
                deadline: None,
                actionability: Actionability::Actionable,
                next_step: next_step.map(str::to_owned),
                dependencies: None,
                confidence: 0.9,
            },
        );
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

        let applied = apply_proposal(&state, &note.id).unwrap();
        let task = applied.task.unwrap();
        assert_eq!(task.title, "Draft the first section");
        assert_eq!(task.estimated_minutes, 25);
        assert_eq!(task.priority, 60);
        assert_eq!(
            task.description.as_deref(),
            Some(format!("Created from Inbox note {}", note.id).as_str())
        );

        let retried_task = apply_proposal(&state, &note.id).unwrap().task.unwrap();
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
            apply_proposal(&state, &note.id),
            Err(AgentError::UnsupportedOperation)
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
            apply_proposal(&state, &note.id),
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

    #[test]
    fn confirmed_project_proposal_creates_project_and_first_task_once() {
        let (directory, state) = test_state();
        let note = inbox::capture(&state, "I want to launch a newsletter").unwrap();
        add_project_proposal(&state, &note.id, Some("Draft the first issue"));

        let applied = apply_proposal(&state, &note.id).unwrap();
        let project = applied.project.unwrap();
        let task = applied.task.unwrap();
        assert_eq!(project.name, "Launch the newsletter");
        assert_eq!(project.description.as_deref(), Some(note.raw_text.as_str()));
        assert_eq!(project.status, "BACKLOG");
        assert_eq!(task.title, "Draft the first issue");
        assert_eq!(task.project_id.as_deref(), Some(project.id.as_str()));

        let retried = apply_proposal(&state, &note.id).unwrap();
        assert_eq!(retried.project.unwrap().id, project.id);
        assert_eq!(retried.task.unwrap().id, task.id);

        let conn = state.conn().unwrap();
        let (project_count, task_count, audit_count, inbox_status): (i64, i64, i64, String) = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM projects WHERE id = ?2),
                    (SELECT COUNT(*) FROM tasks WHERE id = ?3),
                    (SELECT COUNT(*) FROM autonomy_decisions
                     WHERE action_type = 'CREATE_PROJECT_FROM_INBOX'),
                    (SELECT status FROM inbox_items WHERE id = ?1)",
                rusqlite::params![note.id, project.id, task.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(project_count, 1);
        assert_eq!(task_count, 1);
        assert_eq!(audit_count, 1);
        assert_eq!(inbox_status, "CONVERTED");

        drop(conn);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn actionable_project_without_a_next_step_creates_no_task() {
        let (directory, state) = test_state();
        let note = inbox::capture(&state, "I want to organize a community meetup").unwrap();
        add_project_proposal(&state, &note.id, None);

        let applied = apply_proposal(&state, &note.id).unwrap();
        assert!(applied.project.is_some());
        assert!(applied.task.is_none());
        assert!(crate::application::tasks::list_open(&state)
            .unwrap()
            .is_empty());

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn project_and_first_task_roll_back_when_audit_insert_fails() {
        let (directory, state) = test_state();
        let note = inbox::capture(&state, "I want to launch a newsletter").unwrap();
        add_project_proposal(&state, &note.id, Some("Draft the first issue"));
        {
            let conn = state.conn().unwrap();
            conn.execute_batch(
                "CREATE TRIGGER reject_test_project_audit
                 BEFORE INSERT ON autonomy_decisions
                 BEGIN
                     SELECT RAISE(ABORT, 'test audit failure');
                 END;",
            )
            .unwrap();
        }

        assert!(matches!(
            apply_proposal(&state, &note.id),
            Err(AgentError::Database(_))
        ));
        let conn = state.conn().unwrap();
        let (project_count, task_count, proposal_status, inbox_status): (i64, i64, String, String) =
            conn.query_row(
                "SELECT
                    (SELECT COUNT(*) FROM projects),
                    (SELECT COUNT(*) FROM tasks),
                    (SELECT status FROM classification_proposals WHERE inbox_item_id = ?1),
                    (SELECT status FROM inbox_items WHERE id = ?1)",
                [&note.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(project_count, 0);
        assert_eq!(task_count, 0);
        assert_eq!(proposal_status, "PENDING");
        assert_eq!(inbox_status, "PROPOSED");

        drop(conn);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
