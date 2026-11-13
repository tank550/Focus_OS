use rusqlite::{Connection, OptionalExtension};

use crate::infrastructure::db::DbError;

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: u16,
    pub progress: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct ProjectsRepo;

impl ProjectsRepo {
    pub fn get(conn: &Connection, id: &str) -> Result<Option<ProjectRecord>, DbError> {
        Ok(conn
            .query_row(
                "SELECT id, name, description, status, priority, progress, created_at, updated_at
                 FROM projects
                 WHERE id = ?1 AND archived = 0 AND status != 'ARCHIVED'",
                [id],
                |row| {
                    Ok(ProjectRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        status: row.get(3)?,
                        priority: row.get(4)?,
                        progress: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn insert(conn: &Connection, project: &ProjectRecord) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO projects
                (id, name, description, status, priority, progress, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                project.id,
                project.name,
                project.description,
                project.status,
                project.priority,
                project.progress,
                project.created_at,
                project.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list(conn: &Connection) -> Result<Vec<ProjectRecord>, DbError> {
        let mut statement = conn.prepare(
            "SELECT id, name, description, status, priority, progress, created_at, updated_at
             FROM projects
             WHERE archived = 0 AND status != 'ARCHIVED'
             ORDER BY
                CASE status
                    WHEN 'ACTIVE' THEN 0
                    WHEN 'BACKLOG' THEN 1
                    WHEN 'ON_HOLD' THEN 2
                    WHEN 'COMPLETED' THEN 3
                    ELSE 4
                END,
                priority DESC,
                updated_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ProjectRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                status: row.get(3)?,
                priority: row.get(4)?,
                progress: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn update(
        conn: &Connection,
        id: &str,
        name: &str,
        description: Option<&str>,
        status: &str,
        priority: u16,
        updated_at: i64,
    ) -> Result<bool, DbError> {
        let changed = conn.execute(
            "UPDATE projects
             SET name = ?2, description = ?3, status = ?4, priority = ?5, updated_at = ?6
             WHERE id = ?1 AND archived = 0 AND status != 'ARCHIVED'",
            rusqlite::params![id, name, description, status, priority, updated_at],
        )?;
        Ok(changed == 1)
    }

    pub fn archive(conn: &Connection, id: &str, updated_at: i64) -> Result<bool, DbError> {
        let changed = conn.execute(
            "UPDATE projects
             SET archived = 1, status = 'ARCHIVED', updated_at = ?2
             WHERE id = ?1 AND archived = 0 AND status != 'ARCHIVED'",
            rusqlite::params![id, updated_at],
        )?;
        Ok(changed == 1)
    }
}
