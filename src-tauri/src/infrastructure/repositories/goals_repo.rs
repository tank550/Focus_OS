use rusqlite::Connection;

use crate::infrastructure::db::DbError;

#[derive(Debug, Clone, PartialEq)]
pub struct GoalRecord {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub expected_outcome: Option<String>,
    pub success_criteria: Option<String>,
    pub status: String,
    pub progress: f64,
    pub priority: u16,
    pub target_date: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidecarEntryRecord {
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
    pub source_ref: Option<String>,
    pub created_at: i64,
}

pub struct GoalsRepo;

impl GoalsRepo {
    pub fn insert(conn: &Connection, goal: &GoalRecord) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO goals
                (id, title, description, expected_outcome, success_criteria, status,
                 progress, priority, target_date, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                goal.id,
                goal.title,
                goal.description,
                goal.expected_outcome,
                goal.success_criteria,
                goal.status,
                goal.progress,
                goal.priority,
                goal.target_date,
                goal.created_at,
                goal.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list(conn: &Connection) -> Result<Vec<GoalRecord>, DbError> {
        let mut statement = conn.prepare(
            "SELECT id, title, description, expected_outcome, success_criteria, status,
                    progress, priority, target_date, created_at, updated_at
             FROM goals
             WHERE archived = 0 AND status != 'ABANDONED'
             ORDER BY
                CASE status WHEN 'ACTIVE' THEN 0 WHEN 'PLANNED' THEN 1 ELSE 2 END,
                priority DESC, target_date ASC, created_at ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(GoalRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                expected_outcome: row.get(3)?,
                success_criteria: row.get(4)?,
                status: row.get(5)?,
                progress: row.get(6)?,
                priority: row.get(7)?,
                target_date: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn exists(conn: &Connection, goal_id: &str) -> Result<bool, DbError> {
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM goals WHERE id = ?1 AND archived = 0)",
            [goal_id],
            |row| row.get(0),
        )?)
    }

    pub fn insert_sidecar(conn: &Connection, entry: &SidecarEntryRecord) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO goal_sidecar_entries
                (id, goal_id, source, entry_type, content, application_name, window_title,
                 domain, observed_start, observed_end, duration_seconds, confidence,
                 review_status, source_ref, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
            rusqlite::params![
                entry.id,
                entry.goal_id,
                entry.source,
                entry.entry_type,
                entry.content,
                entry.application_name,
                entry.window_title,
                entry.domain,
                entry.observed_start,
                entry.observed_end,
                entry.duration_seconds,
                entry.confidence,
                entry.review_status,
                entry.source_ref,
                entry.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_sidecar(
        conn: &Connection,
        goal_id: &str,
    ) -> Result<Vec<SidecarEntryRecord>, DbError> {
        let mut statement = conn.prepare(
            "SELECT id, goal_id, source, entry_type, content, application_name, window_title,
                    domain, observed_start, observed_end, duration_seconds, confidence,
                    review_status, source_ref, created_at
             FROM goal_sidecar_entries
             WHERE goal_id = ?1 AND review_status != 'REJECTED'
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = statement.query_map([goal_id], |row| {
            Ok(SidecarEntryRecord {
                id: row.get(0)?,
                goal_id: row.get(1)?,
                source: row.get(2)?,
                entry_type: row.get(3)?,
                content: row.get(4)?,
                application_name: row.get(5)?,
                window_title: row.get(6)?,
                domain: row.get(7)?,
                observed_start: row.get(8)?,
                observed_end: row.get(9)?,
                duration_seconds: row.get(10)?,
                confidence: row.get(11)?,
                review_status: row.get(12)?,
                source_ref: row.get(13)?,
                created_at: row.get(14)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
