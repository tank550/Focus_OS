use serde::Serialize;
use thiserror::Error;

use focus_domain::ids::EntityId;
use focus_domain::status::GoalStatus;
use focus_domain::value_objects::UtcMillis;

use crate::infrastructure::db::DbError;
use crate::infrastructure::repositories::goals_repo::{GoalRecord, GoalsRepo, SidecarEntryRecord};
use crate::state::AppState;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub expected_outcome: Option<String>,
    pub success_criteria: Vec<String>,
    pub status: String,
    pub progress: f64,
    pub priority: u16,
    pub target_date: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl TryFrom<GoalRecord> for Goal {
    type Error = GoalsError;

    fn try_from(record: GoalRecord) -> Result<Self, Self::Error> {
        let success_criteria = match record.success_criteria {
            Some(raw) => serde_json::from_str(&raw)
                .map_err(|error| GoalsError::InvalidStoredData(error.to_string()))?,
            None => Vec::new(),
        };
        let status = record
            .status
            .parse::<GoalStatus>()
            .map_err(|error| GoalsError::InvalidStoredData(error.to_string()))?
            .to_string();
        Ok(Self {
            id: record.id,
            title: record.title,
            description: record.description,
            expected_outcome: record.expected_outcome,
            success_criteria,
            status,
            progress: record.progress,
            priority: record.priority,
            target_date: record.target_date,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarEntry {
    pub id: String,
    pub goal_id: String,
    pub source: String,
    pub entry_type: String,
    pub content: String,
    pub application_name: Option<String>,
    pub window_title: Option<String>,
    pub domain: Option<String>,
    pub observed_start: Option<i64>,
    pub observed_end: Option<i64>,
    pub duration_seconds: Option<i64>,
    pub confidence: f64,
    pub review_status: String,
    pub created_at: i64,
}

impl From<SidecarEntryRecord> for SidecarEntry {
    fn from(record: SidecarEntryRecord) -> Self {
        Self {
            id: record.id,
            goal_id: record.goal_id,
            source: record.source,
            entry_type: record.entry_type,
            content: record.content,
            application_name: record.application_name,
            window_title: record.window_title,
            domain: record.domain,
            observed_start: record.observed_start,
            observed_end: record.observed_end,
            duration_seconds: record.duration_seconds,
            confidence: record.confidence,
            review_status: record.review_status,
            created_at: record.created_at,
        }
    }
}

#[derive(Debug, Error)]
pub enum GoalsError {
    #[error("Goal title cannot be empty")]
    EmptyTitle,
    #[error("A check-in cannot be empty")]
    EmptyCheckIn,
    #[error("Goal was not found or is archived")]
    GoalNotFound,
    #[error("Invalid goal data in the database: {0}")]
    InvalidStoredData(String),
    #[error("Could not encode goal success criteria: {0}")]
    InvalidCriteria(String),
    #[error(transparent)]
    Database(#[from] DbError),
}

pub fn list(state: &AppState) -> Result<Vec<Goal>, GoalsError> {
    let conn = state.conn()?;
    GoalsRepo::list(&conn)?
        .into_iter()
        .map(Goal::try_from)
        .collect()
}

pub fn create(
    state: &AppState,
    title: &str,
    description: Option<&str>,
    expected_outcome: Option<&str>,
    success_criteria: &[String],
) -> Result<Goal, GoalsError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(GoalsError::EmptyTitle);
    }
    let description = clean_optional(description);
    let expected_outcome = clean_optional(expected_outcome);
    let success_criteria = success_criteria
        .iter()
        .map(|criterion| criterion.trim())
        .filter(|criterion| !criterion.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let success_criteria_json = if success_criteria.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(&success_criteria)
                .map_err(|error| GoalsError::InvalidCriteria(error.to_string()))?,
        )
    };
    let now = UtcMillis::now().0;
    let record = GoalRecord {
        id: EntityId::new().to_string(),
        title: title.to_owned(),
        description,
        expected_outcome,
        success_criteria: success_criteria_json,
        status: GoalStatus::Planned.to_string(),
        progress: 0.0,
        priority: 0,
        target_date: None,
        created_at: now,
        updated_at: now,
    };
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    GoalsRepo::insert(&tx, &record)?;
    tx.commit().map_err(DbError::from)?;
    Goal::try_from(record)
}

pub fn list_sidecar(state: &AppState, goal_id: &str) -> Result<Vec<SidecarEntry>, GoalsError> {
    let conn = state.conn()?;
    if !GoalsRepo::exists(&conn, goal_id)? {
        return Err(GoalsError::GoalNotFound);
    }
    Ok(GoalsRepo::list_sidecar(&conn, goal_id)?
        .into_iter()
        .map(SidecarEntry::from)
        .collect())
}

pub fn add_check_in(
    state: &AppState,
    goal_id: &str,
    content: &str,
) -> Result<SidecarEntry, GoalsError> {
    if content.trim().is_empty() {
        return Err(GoalsError::EmptyCheckIn);
    }
    let now = UtcMillis::now().0;
    let record = SidecarEntryRecord {
        id: EntityId::new().to_string(),
        goal_id: goal_id.to_owned(),
        source: "USER".into(),
        entry_type: "CHECK_IN".into(),
        content: content.to_owned(),
        application_name: None,
        window_title: None,
        domain: None,
        observed_start: Some(now),
        observed_end: Some(now),
        duration_seconds: None,
        confidence: 1.0,
        review_status: "CONFIRMED".into(),
        source_ref: None,
        created_at: now,
    };
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    if !GoalsRepo::exists(&tx, goal_id)? {
        return Err(GoalsError::GoalNotFound);
    }
    GoalsRepo::insert_sidecar(&tx, &record)?;
    tx.commit().map_err(DbError::from)?;
    Ok(record.into())
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

    fn test_state() -> (std::path::PathBuf, AppState) {
        let directory = std::env::temp_dir().join(format!("focus-os-goals-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        (directory, AppState::new(pool, database_path))
    }

    #[test]
    fn creates_goal_and_records_separate_sidecar_check_in() {
        let (directory, state) = test_state();
        let goal = create(
            &state,
            "  Improve my health ",
            Some("Build sustainable habits"),
            Some("Exercise regularly"),
            &["Move 3 times weekly".into(), " ".into()],
        )
        .unwrap();
        assert_eq!(goal.status, "PLANNED");
        assert_eq!(goal.progress, 0.0);
        assert_eq!(goal.success_criteria, vec!["Move 3 times weekly"]);

        let entry = add_check_in(&state, &goal.id, "Walked for 30 minutes").unwrap();
        assert_eq!(entry.source, "USER");
        assert_eq!(entry.entry_type, "CHECK_IN");
        assert_eq!(entry.review_status, "CONFIRMED");
        assert_eq!(list_sidecar(&state, &goal.id).unwrap(), vec![entry]);

        let conn = state.conn().unwrap();
        let (progress, sidecar_count): (f64, i64) = conn
            .query_row(
                "SELECT g.progress,
                        (SELECT COUNT(*) FROM goal_sidecar_entries s WHERE s.goal_id = g.id)
                 FROM goals g WHERE g.id = ?1",
                [&goal.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            progress, 0.0,
            "observations must not rewrite official progress"
        );
        assert_eq!(sidecar_count, 1);

        drop(conn);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_empty_goals_and_check_ins() {
        let (directory, state) = test_state();
        assert!(matches!(
            create(&state, " ", None, None, &[]),
            Err(GoalsError::EmptyTitle)
        ));
        let goal = create(&state, "Learn Rust", None, None, &[]).unwrap();
        assert!(matches!(
            add_check_in(&state, &goal.id, "  "),
            Err(GoalsError::EmptyCheckIn)
        ));

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
