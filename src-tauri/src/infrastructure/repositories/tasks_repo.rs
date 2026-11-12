use rusqlite::{Connection, OptionalExtension};

use crate::infrastructure::db::DbError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub goal_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: u16,
    pub estimated_minutes: u32,
    pub actual_minutes: u32,
    pub requires_deep_work: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSessionRecord {
    pub id: String,
    pub task_id: String,
    pub goal_id: Option<String>,
    pub task_title: String,
    pub start_time: i64,
}

pub struct TasksRepo;

impl TasksRepo {
    pub fn project_exists(conn: &Connection, project_id: &str) -> Result<bool, DbError> {
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects
             WHERE id = ?1 AND archived = 0 AND status != 'ARCHIVED')",
            [project_id],
            |row| row.get(0),
        )?)
    }

    pub fn goal_exists(conn: &Connection, goal_id: &str) -> Result<bool, DbError> {
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM goals WHERE id = ?1 AND archived = 0)",
            [goal_id],
            |row| row.get(0),
        )?)
    }

    pub fn insert_from_agent(conn: &Connection, task: &TaskRecord) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO tasks
                (id, project_id, goal_id, title, description, status, priority,
                 estimated_minutes, requires_deep_work, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'TODO', ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                task.id,
                task.project_id,
                task.goal_id,
                task.title,
                task.description,
                task.priority,
                task.estimated_minutes,
                task.requires_deep_work,
                task.created_at,
                task.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_open(conn: &Connection) -> Result<Vec<TaskRecord>, DbError> {
        let mut statement = conn.prepare(
            "SELECT id, project_id, goal_id, title, description, status, priority,
                    estimated_minutes, actual_minutes, requires_deep_work, created_at, updated_at
             FROM tasks
             WHERE archived = 0 AND deleted_at IS NULL
               AND status IN ('TODO','SCHEDULED','IN_PROGRESS','PAUSED')
             ORDER BY
                CASE status WHEN 'IN_PROGRESS' THEN 0 WHEN 'PAUSED' THEN 1 ELSE 2 END,
                priority DESC, deadline ASC, created_at ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(TaskRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                goal_id: row.get(2)?,
                title: row.get(3)?,
                description: row.get(4)?,
                status: row.get(5)?,
                priority: row.get(6)?,
                estimated_minutes: row.get(7)?,
                actual_minutes: row.get(8)?,
                requires_deep_work: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get(conn: &Connection, id: &str) -> Result<Option<TaskRecord>, DbError> {
        Ok(conn
            .query_row(
                "SELECT id, project_id, goal_id, title, description, status, priority,
                        estimated_minutes, actual_minutes, requires_deep_work, created_at, updated_at
                 FROM tasks
                 WHERE id = ?1 AND archived = 0 AND deleted_at IS NULL",
                [id],
                |row| {
                    Ok(TaskRecord {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        goal_id: row.get(2)?,
                        title: row.get(3)?,
                        description: row.get(4)?,
                        status: row.get(5)?,
                        priority: row.get(6)?,
                        estimated_minutes: row.get(7)?,
                        actual_minutes: row.get(8)?,
                        requires_deep_work: row.get(9)?,
                        created_at: row.get(10)?,
                        updated_at: row.get(11)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn has_active_session(conn: &Connection) -> Result<bool, DbError> {
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_sessions WHERE end_time IS NULL)",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn active_session_for_task(
        conn: &Connection,
        task_id: &str,
    ) -> Result<Option<ActiveSessionRecord>, DbError> {
        Ok(conn
            .query_row(
                "SELECT s.id, s.task_id, t.goal_id, t.title, s.start_time
                 FROM task_sessions s
                 JOIN tasks t ON t.id = s.task_id
                 WHERE s.task_id = ?1 AND s.end_time IS NULL
                 ORDER BY s.start_time DESC LIMIT 1",
                [task_id],
                |row| {
                    Ok(ActiveSessionRecord {
                        id: row.get(0)?,
                        task_id: row.get(1)?,
                        goal_id: row.get(2)?,
                        task_title: row.get(3)?,
                        start_time: row.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn active_session(conn: &Connection) -> Result<Option<ActiveSessionRecord>, DbError> {
        Ok(conn
            .query_row(
                "SELECT s.id, s.task_id, t.goal_id, t.title, s.start_time
                 FROM task_sessions s
                 JOIN tasks t ON t.id = s.task_id
                 WHERE s.end_time IS NULL
                 ORDER BY s.start_time DESC LIMIT 1",
                [],
                |row| {
                    Ok(ActiveSessionRecord {
                        id: row.get(0)?,
                        task_id: row.get(1)?,
                        goal_id: row.get(2)?,
                        task_title: row.get(3)?,
                        start_time: row.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn start_session(
        conn: &Connection,
        id: &str,
        task_id: &str,
        started_at: i64,
    ) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO task_sessions
                (id, task_id, start_time, duration_minutes, session_type, created_at, updated_at)
             VALUES (?1, ?2, ?3, 0, 'FOCUS', ?3, ?3)",
            rusqlite::params![id, task_id, started_at],
        )?;
        Ok(())
    }

    pub fn set_status(
        conn: &Connection,
        task_id: &str,
        status: &str,
        updated_at: i64,
        completed_at: Option<i64>,
    ) -> Result<bool, DbError> {
        let changed = conn.execute(
            "UPDATE tasks
             SET status = ?2, updated_at = ?3, completed_at = COALESCE(?4, completed_at)
             WHERE id = ?1 AND archived = 0 AND deleted_at IS NULL",
            rusqlite::params![task_id, status, updated_at, completed_at],
        )?;
        Ok(changed == 1)
    }

    pub fn finish_session(
        conn: &Connection,
        session_id: &str,
        task_id: &str,
        ended_at: i64,
        duration_minutes: u32,
        next_status: &str,
        completed_at: Option<i64>,
    ) -> Result<bool, DbError> {
        let changed = conn.execute(
            "UPDATE task_sessions
             SET end_time = ?3, duration_minutes = ?4, updated_at = ?3
             WHERE id = ?1 AND task_id = ?2 AND end_time IS NULL",
            rusqlite::params![session_id, task_id, ended_at, duration_minutes],
        )?;
        if changed != 1 {
            return Ok(false);
        }
        conn.execute(
            "UPDATE tasks
             SET actual_minutes = actual_minutes + ?2, status = ?3, updated_at = ?4,
                 completed_at = COALESCE(?5, completed_at)
             WHERE id = ?1",
            rusqlite::params![
                task_id,
                duration_minutes,
                next_status,
                ended_at,
                completed_at
            ],
        )?;
        Ok(true)
    }
}
