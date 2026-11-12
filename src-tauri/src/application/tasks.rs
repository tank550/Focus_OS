use serde::Serialize;
use thiserror::Error;

use focus_domain::ids::EntityId;
use focus_domain::status::TaskStatus;
use focus_domain::value_objects::UtcMillis;

use crate::infrastructure::db::DbError;
use crate::infrastructure::repositories::goals_repo::{GoalsRepo, SidecarEntryRecord};
use crate::infrastructure::repositories::tasks_repo::{ActiveSessionRecord, TaskRecord, TasksRepo};
use crate::state::AppState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub project_id: Option<String>,
    pub goal_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: u16,
    pub estimated_minutes: u32,
    pub actual_minutes: u32,
    pub requires_deep_work: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl TryFrom<TaskRecord> for Task {
    type Error = TasksError;

    fn try_from(record: TaskRecord) -> Result<Self, Self::Error> {
        let status = record
            .status
            .parse::<TaskStatus>()
            .map_err(|error| TasksError::InvalidStoredData(error.to_string()))?;
        Ok(Self {
            id: record.id,
            project_id: record.project_id,
            goal_id: record.goal_id,
            title: record.title,
            description: record.description,
            status,
            priority: record.priority,
            estimated_minutes: record.estimated_minutes,
            actual_minutes: record.actual_minutes,
            requires_deep_work: record.requires_deep_work,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSession {
    pub id: String,
    pub task_id: String,
    pub task_title: String,
    pub start_time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTaskDraft {
    pub title: String,
    pub description: Option<String>,
    pub project_id: Option<String>,
    pub goal_id: Option<String>,
    pub estimated_minutes: u32,
    pub priority: u16,
    pub requires_deep_work: bool,
}

impl From<ActiveSessionRecord> for FocusSession {
    fn from(record: ActiveSessionRecord) -> Self {
        Self {
            id: record.id,
            task_id: record.task_id,
            task_title: record.task_title,
            start_time: record.start_time,
        }
    }
}

#[derive(Debug, Error)]
pub enum TasksError {
    #[error("Task title cannot be empty")]
    EmptyTitle,
    #[error("Estimated duration must be positive")]
    InvalidDuration,
    #[error("Priority must be between 0 and 100")]
    InvalidPriority,
    #[error("Linked project or goal was not found or is archived")]
    InvalidLink,
    #[error("Task was not found or is archived")]
    NotFound,
    #[error("Task cannot start from its current state")]
    CannotStart,
    #[error("Another task already has an active focus session")]
    FocusSessionAlreadyActive,
    #[error("This task does not have an active focus session")]
    NotFocusing,
    #[error("Task is already complete or cancelled")]
    AlreadyFinished,
    #[error("Invalid task data in the database: {0}")]
    InvalidStoredData(String),
    #[error(transparent)]
    Database(#[from] DbError),
}

/// Only the note agent calls this service to create tasks; no manual create IPC
/// command is exposed to the webview.
pub fn create_from_agent(state: &AppState, draft: AgentTaskDraft) -> Result<Task, TasksError> {
    let title = draft.title.trim();
    if title.is_empty() {
        return Err(TasksError::EmptyTitle);
    }
    if draft.estimated_minutes == 0 {
        return Err(TasksError::InvalidDuration);
    }
    if draft.priority > 100 {
        return Err(TasksError::InvalidPriority);
    }
    let description = clean_optional(draft.description.as_deref());
    let now = UtcMillis::now().0;
    let record = TaskRecord {
        id: EntityId::new().to_string(),
        project_id: draft.project_id,
        goal_id: draft.goal_id,
        title: title.to_owned(),
        description,
        status: TaskStatus::Todo.to_string(),
        priority: draft.priority,
        estimated_minutes: draft.estimated_minutes,
        actual_minutes: 0,
        requires_deep_work: draft.requires_deep_work,
        created_at: now,
        updated_at: now,
    };

    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    if record
        .project_id
        .as_deref()
        .map(|id| TasksRepo::project_exists(&tx, id))
        .transpose()?
        == Some(false)
        || record
            .goal_id
            .as_deref()
            .map(|id| TasksRepo::goal_exists(&tx, id))
            .transpose()?
            == Some(false)
    {
        return Err(TasksError::InvalidLink);
    }
    TasksRepo::insert_from_agent(&tx, &record)?;
    tx.commit().map_err(DbError::from)?;
    Task::try_from(record)
}

pub fn list_open(state: &AppState) -> Result<Vec<Task>, TasksError> {
    let conn = state.conn()?;
    TasksRepo::list_open(&conn)?
        .into_iter()
        .map(Task::try_from)
        .collect()
}

pub fn current_focus(state: &AppState) -> Result<Option<FocusSession>, TasksError> {
    let conn = state.conn()?;
    Ok(TasksRepo::active_session(&conn)?.map(FocusSession::from))
}

pub fn start_focus(state: &AppState, task_id: &str) -> Result<FocusSession, TasksError> {
    let now = UtcMillis::now().0;
    let mut conn = state.conn()?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(DbError::from)?;
    if TasksRepo::has_active_session(&tx)? {
        return Err(TasksError::FocusSessionAlreadyActive);
    }
    let task = TasksRepo::get(&tx, task_id)?.ok_or(TasksError::NotFound)?;
    let status = task
        .status
        .parse::<TaskStatus>()
        .map_err(|_| TasksError::CannotStart)?;
    if !matches!(
        status,
        TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Paused
    ) {
        return Err(TasksError::CannotStart);
    }

    let session_id = EntityId::new().to_string();
    TasksRepo::start_session(&tx, &session_id, task_id, now)?;
    if !TasksRepo::set_status(&tx, task_id, "IN_PROGRESS", now, None)? {
        return Err(TasksError::NotFound);
    }
    tx.commit().map_err(DbError::from)?;
    Ok(FocusSession {
        id: session_id,
        task_id: task.id,
        task_title: task.title,
        start_time: now,
    })
}

pub fn pause_focus(state: &AppState, task_id: &str) -> Result<(), TasksError> {
    finish_focus(state, task_id, TaskStatus::Paused, None)
}

pub fn complete(state: &AppState, task_id: &str) -> Result<(), TasksError> {
    let now = UtcMillis::now().0;
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    let task = TasksRepo::get(&tx, task_id)?.ok_or(TasksError::NotFound)?;
    let status = task
        .status
        .parse::<TaskStatus>()
        .map_err(|_| TasksError::AlreadyFinished)?;
    if matches!(status, TaskStatus::Completed | TaskStatus::Cancelled) {
        return Err(TasksError::AlreadyFinished);
    }

    if let Some(session) = TasksRepo::active_session_for_task(&tx, task_id)? {
        finish_session_in_transaction(&tx, &session, &task, now, TaskStatus::Completed, Some(now))?;
    } else if !TasksRepo::set_status(
        &tx,
        task_id,
        &TaskStatus::Completed.to_string(),
        now,
        Some(now),
    )? {
        return Err(TasksError::NotFound);
    }

    tx.commit().map_err(DbError::from)?;
    Ok(())
}

fn finish_focus(
    state: &AppState,
    task_id: &str,
    next_status: TaskStatus,
    completed_at: Option<i64>,
) -> Result<(), TasksError> {
    let now = UtcMillis::now().0;
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    let session =
        TasksRepo::active_session_for_task(&tx, task_id)?.ok_or(TasksError::NotFocusing)?;
    let task = TasksRepo::get(&tx, task_id)?.ok_or(TasksError::NotFound)?;
    finish_session_in_transaction(&tx, &session, &task, now, next_status, completed_at)?;
    tx.commit().map_err(DbError::from)?;
    Ok(())
}

fn finish_session_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    session: &ActiveSessionRecord,
    task: &TaskRecord,
    ended_at: i64,
    next_status: TaskStatus,
    completed_at: Option<i64>,
) -> Result<(), TasksError> {
    let duration_seconds = ended_at.saturating_sub(session.start_time) / 1000;
    let duration_minutes = (duration_seconds / 60) as u32;
    if !TasksRepo::finish_session(
        tx,
        &session.id,
        &session.task_id,
        ended_at,
        duration_minutes,
        &next_status.to_string(),
        completed_at,
    )? {
        return Err(TasksError::NotFocusing);
    }

    if let Some(goal_id) = &task.goal_id {
        let entry = SidecarEntryRecord {
            id: EntityId::new().to_string(),
            goal_id: goal_id.clone(),
            source: "FOCUS_OS".into(),
            entry_type: "ACTIVITY".into(),
            content: format!("Focused on task: {}", task.title),
            application_name: None,
            window_title: None,
            domain: None,
            observed_start: Some(session.start_time),
            observed_end: Some(ended_at),
            duration_seconds: Some(duration_seconds),
            confidence: 1.0,
            review_status: "CONFIRMED".into(),
            source_ref: Some(session.id.clone()),
            created_at: ended_at,
        };
        GoalsRepo::insert_sidecar(tx, &entry)?;
    }
    Ok(())
}

fn clean_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::goals;

    fn test_state() -> (std::path::PathBuf, AppState) {
        let directory = std::env::temp_dir().join(format!("focus-os-tasks-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        (directory, AppState::new(pool, database_path))
    }

    #[test]
    fn focus_sessions_update_task_and_write_goal_sidecar_atomically() {
        let (directory, state) = test_state();
        let goal = goals::create(&state, "Ship Focus OS", None, None, &[]).unwrap();
        let task = create_from_agent(
            &state,
            AgentTaskDraft {
                title: "Implement the next action".into(),
                description: None,
                project_id: None,
                goal_id: Some(goal.id.clone()),
                estimated_minutes: 25,
                priority: 60,
                requires_deep_work: true,
            },
        )
        .unwrap();
        let second_task = create_from_agent(
            &state,
            AgentTaskDraft {
                title: "Another action".into(),
                description: None,
                project_id: None,
                goal_id: None,
                estimated_minutes: 15,
                priority: 20,
                requires_deep_work: false,
            },
        )
        .unwrap();

        let session = start_focus(&state, &task.id).unwrap();
        assert_eq!(current_focus(&state).unwrap(), Some(session.clone()));
        assert!(matches!(
            start_focus(&state, &second_task.id),
            Err(TasksError::FocusSessionAlreadyActive)
        ));
        pause_focus(&state, &task.id).unwrap();
        assert_eq!(current_focus(&state).unwrap(), None);
        assert_eq!(list_open(&state).unwrap()[0].status, TaskStatus::Paused);

        start_focus(&state, &task.id).unwrap();
        complete(&state, &task.id).unwrap();
        let remaining_tasks = list_open(&state).unwrap();
        assert_eq!(remaining_tasks.len(), 1);
        assert_eq!(remaining_tasks[0].id, second_task.id);
        assert_eq!(current_focus(&state).unwrap(), None);

        let entries = goals::list_sidecar(&state, &goal.id).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|entry| entry.source == "FOCUS_OS"));
        assert!(entries.iter().all(|entry| entry.entry_type == "ACTIVITY"));
        assert!(entries.iter().all(|entry| entry.duration_seconds.is_some()));

        let conn = state.conn().unwrap();
        let (progress, actual_minutes, session_minutes, completed_at): (
            f64,
            u32,
            u32,
            Option<i64>,
        ) = conn
            .query_row(
                "SELECT g.progress, t.actual_minutes,
                        (SELECT COALESCE(SUM(duration_minutes), 0)
                         FROM task_sessions WHERE task_id = t.id),
                        t.completed_at
                 FROM goals g JOIN tasks t ON t.goal_id = g.id WHERE t.id = ?1",
                [&task.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            progress, 0.0,
            "focus evidence must not change official goal progress"
        );
        assert_eq!(actual_minutes, session_minutes);
        assert!(completed_at.is_some());

        drop(conn);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn sidecar_failure_rolls_back_session_and_task_updates() {
        let (directory, state) = test_state();
        let goal = goals::create(&state, "Ship Focus OS", None, None, &[]).unwrap();
        let task = create_from_agent(
            &state,
            AgentTaskDraft {
                title: "Implement the next action".into(),
                description: None,
                project_id: None,
                goal_id: Some(goal.id.clone()),
                estimated_minutes: 25,
                priority: 60,
                requires_deep_work: false,
            },
        )
        .unwrap();
        start_focus(&state, &task.id).unwrap();
        {
            let conn = state.conn().unwrap();
            conn.execute_batch(
                "CREATE TRIGGER reject_test_sidecar
                 BEFORE INSERT ON goal_sidecar_entries
                 BEGIN
                     SELECT RAISE(ABORT, 'test sidecar failure');
                 END;",
            )
            .unwrap();
        }

        assert!(matches!(
            pause_focus(&state, &task.id),
            Err(TasksError::Database(_))
        ));
        let active = current_focus(&state).unwrap().unwrap();
        assert_eq!(active.task_id, task.id);
        assert_eq!(list_open(&state).unwrap()[0].status, TaskStatus::InProgress);
        assert!(goals::list_sidecar(&state, &goal.id).unwrap().is_empty());

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn agent_task_creation_validates_content_and_goal_reference() {
        let (directory, state) = test_state();
        assert!(matches!(
            create_from_agent(
                &state,
                AgentTaskDraft {
                    title: " ".into(),
                    description: None,
                    project_id: None,
                    goal_id: None,
                    estimated_minutes: 10,
                    priority: 10,
                    requires_deep_work: false,
                }
            ),
            Err(TasksError::EmptyTitle)
        ));
        assert!(matches!(
            create_from_agent(
                &state,
                AgentTaskDraft {
                    title: "Do work".into(),
                    description: None,
                    project_id: None,
                    goal_id: Some("missing".into()),
                    estimated_minutes: 10,
                    priority: 10,
                    requires_deep_work: false,
                }
            ),
            Err(TasksError::InvalidLink)
        ));

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
