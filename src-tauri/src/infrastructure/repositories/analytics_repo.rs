use rusqlite::Connection;

use crate::infrastructure::db::DbError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFocusTimeRecord {
    pub project_id: Option<String>,
    pub project_name: String,
    pub focus_minutes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentFocusSessionRecord {
    pub id: String,
    pub task_title: String,
    pub project_name: Option<String>,
    pub start_time: i64,
    pub duration_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTotalsRecord {
    pub focus_minutes: i64,
    pub session_count: i64,
}

pub struct AnalyticsRepo;

impl AnalyticsRepo {
    pub fn focus_totals(conn: &Connection, since: i64) -> Result<FocusTotalsRecord, DbError> {
        Ok(conn.query_row(
            "SELECT COALESCE(SUM(duration_minutes), 0), COUNT(*)
             FROM task_sessions
             WHERE end_time IS NOT NULL AND start_time >= ?1 AND duration_minutes > 0",
            [since],
            |row| {
                Ok(FocusTotalsRecord {
                    focus_minutes: row.get(0)?,
                    session_count: row.get(1)?,
                })
            },
        )?)
    }

    pub fn completed_tasks_since(conn: &Connection, since: i64) -> Result<i64, DbError> {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE status = 'COMPLETED' AND completed_at >= ?1",
            [since],
            |row| row.get(0),
        )?)
    }

    pub fn focus_time_by_project(
        conn: &Connection,
        since: i64,
    ) -> Result<Vec<ProjectFocusTimeRecord>, DbError> {
        let mut statement = conn.prepare(
            "SELECT p.id, COALESCE(p.name, 'Sans projet'), SUM(s.duration_minutes)
             FROM task_sessions s
             JOIN tasks t ON t.id = s.task_id
             LEFT JOIN projects p ON p.id = t.project_id
             WHERE s.end_time IS NOT NULL AND s.start_time >= ?1
               AND s.duration_minutes > 0
             GROUP BY p.id, p.name
             ORDER BY SUM(s.duration_minutes) DESC, COALESCE(p.name, 'Sans projet')",
        )?;
        let rows = statement.query_map([since], |row| {
            Ok(ProjectFocusTimeRecord {
                project_id: row.get(0)?,
                project_name: row.get(1)?,
                focus_minutes: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn recent_focus_sessions(
        conn: &Connection,
        since: i64,
    ) -> Result<Vec<RecentFocusSessionRecord>, DbError> {
        let mut statement = conn.prepare(
            "SELECT s.id, t.title, p.name, s.start_time, s.duration_minutes
             FROM task_sessions s
             JOIN tasks t ON t.id = s.task_id
             LEFT JOIN projects p ON p.id = t.project_id
             WHERE s.end_time IS NOT NULL AND s.start_time >= ?1
               AND s.duration_minutes > 0
             ORDER BY s.start_time DESC, s.id DESC
             LIMIT 10",
        )?;
        let rows = statement.query_map([since], |row| {
            Ok(RecentFocusSessionRecord {
                id: row.get(0)?,
                task_title: row.get(1)?,
                project_name: row.get(2)?,
                start_time: row.get(3)?,
                duration_minutes: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
