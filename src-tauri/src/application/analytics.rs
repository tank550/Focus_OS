use serde::Serialize;
use thiserror::Error;

use focus_domain::value_objects::UtcMillis;

use crate::infrastructure::db::DbError;
use crate::infrastructure::repositories::analytics_repo::{
    AnalyticsRepo, ProjectFocusTimeRecord, RecentFocusSessionRecord,
};
use crate::state::AppState;

const ANALYTICS_PERIOD_MILLIS: i64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSummary {
    pub period_start: i64,
    pub focus_minutes: i64,
    pub session_count: i64,
    pub average_session_minutes: i64,
    pub completed_tasks: i64,
    pub project_focus_times: Vec<ProjectFocusTime>,
    pub recent_sessions: Vec<RecentFocusSession>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFocusTime {
    pub project_id: Option<String>,
    pub project_name: String,
    pub focus_minutes: u64,
}

impl From<ProjectFocusTimeRecord> for ProjectFocusTime {
    fn from(record: ProjectFocusTimeRecord) -> Self {
        Self {
            project_id: record.project_id,
            project_name: record.project_name,
            focus_minutes: record.focus_minutes.max(0) as u64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFocusSession {
    pub id: String,
    pub task_title: String,
    pub project_name: Option<String>,
    pub start_time: i64,
    pub duration_minutes: u32,
}

impl From<RecentFocusSessionRecord> for RecentFocusSession {
    fn from(record: RecentFocusSessionRecord) -> Self {
        Self {
            id: record.id,
            task_title: record.task_title,
            project_name: record.project_name,
            start_time: record.start_time,
            duration_minutes: record.duration_minutes,
        }
    }
}

#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error(transparent)]
    Database(#[from] DbError),
}

pub fn summary(state: &AppState) -> Result<AnalyticsSummary, AnalyticsError> {
    let period_start = UtcMillis::now().0.saturating_sub(ANALYTICS_PERIOD_MILLIS);
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;

    let totals = AnalyticsRepo::focus_totals(&tx, period_start)?;
    let completed_tasks = AnalyticsRepo::completed_tasks_since(&tx, period_start)?;
    let project_focus_times = AnalyticsRepo::focus_time_by_project(&tx, period_start)?
        .into_iter()
        .map(ProjectFocusTime::from)
        .collect();
    let recent_sessions = AnalyticsRepo::recent_focus_sessions(&tx, period_start)?
        .into_iter()
        .map(RecentFocusSession::from)
        .collect();

    tx.commit().map_err(DbError::from)?;
    Ok(AnalyticsSummary {
        period_start,
        focus_minutes: totals.focus_minutes,
        session_count: totals.session_count,
        average_session_minutes: if totals.session_count == 0 {
            0
        } else {
            totals.focus_minutes / totals.session_count
        },
        completed_tasks,
        project_focus_times,
        recent_sessions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use focus_domain::ids::EntityId;

    fn test_state() -> (std::path::PathBuf, AppState) {
        let directory =
            std::env::temp_dir().join(format!("focus-os-analytics-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        (directory, AppState::new(pool, database_path))
    }

    #[test]
    fn summary_includes_closed_recent_sessions_and_completed_tasks() {
        let (directory, state) = test_state();
        let now = UtcMillis::now().0;
        let conn = state.conn().unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, created_at, updated_at)
             VALUES ('project-1', 'Projet A', ?1, ?1)",
            [now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks
                (id, project_id, title, status, estimated_minutes, completed_at, created_at, updated_at)
             VALUES
                ('task-1', 'project-1', 'Terminer le rapport', 'COMPLETED', 30, ?1, ?1, ?1),
                ('task-2', NULL, 'Tâche sans projet', 'TODO', 20, NULL, ?1, ?1)",
            [now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_sessions
                (id, task_id, start_time, end_time, duration_minutes, created_at, updated_at)
             VALUES
                ('session-1', 'task-1', ?1, ?2, 25, ?1, ?2),
                ('session-2', 'task-1', ?3, NULL, 0, ?3, ?3)",
            rusqlite::params![now - 60_000, now, now],
        )
        .unwrap();
        drop(conn);

        let result = summary(&state).unwrap();
        assert_eq!(result.focus_minutes, 25);
        assert_eq!(result.session_count, 1);
        assert_eq!(result.average_session_minutes, 25);
        assert_eq!(result.completed_tasks, 1);
        assert_eq!(result.project_focus_times.len(), 1);
        assert_eq!(result.project_focus_times[0].project_name, "Projet A");
        assert_eq!(result.project_focus_times[0].focus_minutes, 25);
        assert_eq!(result.recent_sessions.len(), 1);
        assert_eq!(result.recent_sessions[0].task_title, "Terminer le rapport");

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn summary_returns_zero_totals_when_there_is_no_activity() {
        let (directory, state) = test_state();

        let result = summary(&state).unwrap();

        assert_eq!(result.focus_minutes, 0);
        assert_eq!(result.session_count, 0);
        assert_eq!(result.average_session_minutes, 0);
        assert_eq!(result.completed_tasks, 0);
        assert!(result.project_focus_times.is_empty());
        assert!(result.recent_sessions.is_empty());

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
