use serde::Serialize;
use thiserror::Error;

use focus_domain::ids::EntityId;
use focus_domain::status::ProjectStatus;
use focus_domain::value_objects::UtcMillis;

use crate::infrastructure::db::DbError;
use crate::infrastructure::repositories::projects_repo::{ProjectRecord, ProjectsRepo};
use crate::state::AppState;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: u16,
    pub progress: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<ProjectRecord> for Project {
    fn from(record: ProjectRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            description: record.description,
            status: record.status,
            priority: record.priority,
            progress: record.progress,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Debug, Error)]
pub enum ProjectsError {
    #[error("Project name cannot be empty")]
    EmptyName,
    #[error("Project priority must be between 0 and 100")]
    InvalidPriority,
    #[error("Invalid project status: {0}")]
    InvalidStatus(String),
    #[error("Project was not found or is no longer active")]
    NotFound,
    #[error(transparent)]
    Database(#[from] DbError),
}

fn validate_input(
    name: &str,
    status: &str,
    priority: u16,
) -> Result<(String, String), ProjectsError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ProjectsError::EmptyName);
    }
    if priority > 100 {
        return Err(ProjectsError::InvalidPriority);
    }
    let parsed_status = status
        .parse::<ProjectStatus>()
        .map_err(|_| ProjectsError::InvalidStatus(status.to_owned()))?;
    if parsed_status == ProjectStatus::Archived {
        return Err(ProjectsError::InvalidStatus(status.to_owned()));
    }
    Ok((name.to_owned(), parsed_status.to_string()))
}

pub fn list(state: &AppState) -> Result<Vec<Project>, ProjectsError> {
    let conn = state.conn()?;
    Ok(ProjectsRepo::list(&conn)?
        .into_iter()
        .map(Project::from)
        .collect())
}

pub fn create(
    state: &AppState,
    name: &str,
    description: Option<&str>,
    status: &str,
    priority: u16,
) -> Result<Project, ProjectsError> {
    let (name, status) = validate_input(name, status, priority)?;
    let description = description
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let now = UtcMillis::now().0;
    let project = Project {
        id: EntityId::new().to_string(),
        name,
        description,
        status,
        priority,
        progress: 0.0,
        created_at: now,
        updated_at: now,
    };

    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    ProjectsRepo::insert(
        &tx,
        &ProjectRecord {
            id: project.id.clone(),
            name: project.name.clone(),
            description: project.description.clone(),
            status: project.status.clone(),
            priority: project.priority,
            progress: project.progress,
            created_at: project.created_at,
            updated_at: project.updated_at,
        },
    )?;
    tx.commit().map_err(DbError::from)?;
    Ok(project)
}

pub fn update(
    state: &AppState,
    id: &str,
    name: &str,
    description: Option<&str>,
    status: &str,
    priority: u16,
) -> Result<(), ProjectsError> {
    let (name, status) = validate_input(name, status, priority)?;
    let description = description
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    if !ProjectsRepo::update(
        &tx,
        id,
        &name,
        description.as_deref(),
        &status,
        priority,
        UtcMillis::now().0,
    )? {
        return Err(ProjectsError::NotFound);
    }
    tx.commit().map_err(DbError::from)?;
    Ok(())
}

pub fn archive(state: &AppState, id: &str) -> Result<(), ProjectsError> {
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    if !ProjectsRepo::archive(&tx, id, UtcMillis::now().0)? {
        return Err(ProjectsError::NotFound);
    }
    tx.commit().map_err(DbError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> (std::path::PathBuf, AppState) {
        let directory = std::env::temp_dir().join(format!("focus-os-projects-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        (directory, AppState::new(pool, database_path))
    }

    #[test]
    fn project_crud_persists_updates_and_archives_without_deleting() {
        let (directory, state) = test_state();
        let project = create(
            &state,
            "  Write the book  ",
            Some(" First draft "),
            "ACTIVE",
            70,
        )
        .unwrap();
        assert_eq!(project.name, "Write the book");
        assert_eq!(project.description.as_deref(), Some("First draft"));
        assert_eq!(list(&state).unwrap(), vec![project.clone()]);

        update(
            &state,
            &project.id,
            "Write and publish the book",
            Some(""),
            "ON_HOLD",
            85,
        )
        .unwrap();
        let updated = list(&state).unwrap().remove(0);
        assert_eq!(updated.name, "Write and publish the book");
        assert_eq!(updated.description, None);
        assert_eq!(updated.status, "ON_HOLD");
        assert_eq!(updated.priority, 85);

        archive(&state, &project.id).unwrap();
        assert!(list(&state).unwrap().is_empty());
        let conn = state.conn().unwrap();
        let (name, archived, status): (String, i64, String) = conn
            .query_row(
                "SELECT name, archived, status FROM projects WHERE id = ?1",
                [&project.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "Write and publish the book");
        assert_eq!(archived, 1);
        assert_eq!(status, "ARCHIVED");

        drop(conn);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn project_input_validation_rejects_empty_name_bad_status_and_priority() {
        let (directory, state) = test_state();
        assert!(matches!(
            create(&state, " \n ", None, "BACKLOG", 10),
            Err(ProjectsError::EmptyName)
        ));
        assert!(matches!(
            create(&state, "A project", None, "UNKNOWN", 10),
            Err(ProjectsError::InvalidStatus(_))
        ));
        assert!(matches!(
            create(&state, "A project", None, "BACKLOG", 101),
            Err(ProjectsError::InvalidPriority)
        ));

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
