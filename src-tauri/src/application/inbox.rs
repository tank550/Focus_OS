use serde::Serialize;
use thiserror::Error;

use focus_domain::ids::EntityId;
use focus_domain::value_objects::UtcMillis;

use crate::infrastructure::db::DbError;
use crate::infrastructure::repositories::inbox_repo::{InboxItemRecord, InboxRepo};
use crate::state::AppState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    pub raw_text: String,
    pub kind: String,
    pub status: String,
    pub captured_at: i64,
}

impl From<InboxItemRecord> for InboxItem {
    fn from(record: InboxItemRecord) -> Self {
        Self {
            id: record.id,
            raw_text: record.raw_text,
            kind: record.kind,
            status: record.status,
            captured_at: record.captured_at,
        }
    }
}

#[derive(Debug, Error)]
pub enum InboxError {
    #[error("Inbox text cannot be empty")]
    EmptyText,
    #[error("Inbox note was not found or is no longer active")]
    NotFound,
    #[error(transparent)]
    Database(#[from] DbError),
}

pub fn capture(state: &AppState, raw_text: &str) -> Result<InboxItem, InboxError> {
    if raw_text.trim().is_empty() {
        return Err(InboxError::EmptyText);
    }

    let captured_at = UtcMillis::now().0;
    let item = InboxItem {
        id: EntityId::new().to_string(),
        raw_text: raw_text.to_owned(),
        kind: "UNKNOWN".into(),
        status: "RAW".into(),
        captured_at,
    };
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    InboxRepo::insert(
        &tx,
        &InboxItemRecord {
            id: item.id.clone(),
            raw_text: item.raw_text.clone(),
            kind: item.kind.clone(),
            status: item.status.clone(),
            captured_at: item.captured_at,
        },
    )?;
    tx.commit().map_err(DbError::from)?;

    Ok(item)
}

pub fn list(state: &AppState) -> Result<Vec<InboxItem>, InboxError> {
    let conn = state.conn()?;
    Ok(InboxRepo::list(&conn)?
        .into_iter()
        .map(InboxItem::from)
        .collect())
}

pub fn update(state: &AppState, id: &str, raw_text: &str) -> Result<(), InboxError> {
    if raw_text.trim().is_empty() {
        return Err(InboxError::EmptyText);
    }

    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    if !InboxRepo::update(&tx, id, raw_text, UtcMillis::now().0)? {
        return Err(InboxError::NotFound);
    }
    tx.commit().map_err(DbError::from)?;
    Ok(())
}

pub fn archive(state: &AppState, id: &str) -> Result<(), InboxError> {
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    if !InboxRepo::archive(&tx, id, UtcMillis::now().0)? {
        return Err(InboxError::NotFound);
    }
    tx.commit().map_err(DbError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_preserves_multiline_text_and_persists_inbox_item() {
        let directory = std::env::temp_dir().join(format!("focus-os-inbox-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        let state = AppState::new(pool, database_path);
        let text = "  Review the proposal\n\nThen write down the open questions.  ";

        let captured = capture(&state, text).unwrap();
        assert_eq!(captured.raw_text, text);
        assert_eq!(captured.kind, "UNKNOWN");
        assert_eq!(captured.status, "RAW");
        assert_eq!(list(&state).unwrap(), vec![captured]);

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn capture_rejects_blank_text_without_database_access() {
        let directory = std::env::temp_dir().join(format!("focus-os-inbox-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        let state = AppState::new(pool, database_path);

        assert!(matches!(
            capture(&state, " \n\t "),
            Err(InboxError::EmptyText)
        ));

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn update_persists_the_full_note_text() {
        let directory = std::env::temp_dir().join(format!("focus-os-inbox-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        let state = AppState::new(pool, database_path);
        let captured = capture(&state, "Initial thought").unwrap();
        let edited = "A longer thought.\n\nWith separate paragraphs.";

        update(&state, &captured.id, edited).unwrap();

        let items = list(&state).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, captured.id);
        assert_eq!(items[0].raw_text, edited);

        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn archive_hides_note_without_deleting_it() {
        let directory = std::env::temp_dir().join(format!("focus-os-inbox-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        let state = AppState::new(pool, database_path);
        let captured = capture(&state, "Keep this note recoverable").unwrap();

        archive(&state, &captured.id).unwrap();
        assert!(list(&state).unwrap().is_empty());

        let conn = state.conn().unwrap();
        let (raw_text, archived, status): (String, i64, String) = conn
            .query_row(
                "SELECT raw_text, archived, status FROM inbox_items WHERE id = ?1",
                [&captured.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(raw_text, "Keep this note recoverable");
        assert_eq!(archived, 1);
        assert_eq!(status, "ARCHIVED");

        drop(conn);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
