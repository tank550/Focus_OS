use rusqlite::Connection;

use crate::infrastructure::db::DbError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxItemRecord {
    pub id: String,
    pub raw_text: String,
    pub kind: String,
    pub status: String,
    pub captured_at: i64,
}

pub struct InboxRepo;

impl InboxRepo {
    pub fn insert(conn: &Connection, item: &InboxItemRecord) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO inbox_items
                (id, raw_text, kind, status, captured_at, created_at, updated_at, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5, 'manual')",
            rusqlite::params![
                item.id,
                item.raw_text,
                item.kind,
                item.status,
                item.captured_at
            ],
        )?;
        Ok(())
    }

    pub fn list(conn: &Connection) -> Result<Vec<InboxItemRecord>, DbError> {
        let mut statement = conn.prepare(
            "SELECT id, raw_text, kind, status, captured_at
             FROM inbox_items
             WHERE archived = 0 AND status != 'ARCHIVED'
             ORDER BY captured_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(InboxItemRecord {
                id: row.get(0)?,
                raw_text: row.get(1)?,
                kind: row.get(2)?,
                status: row.get(3)?,
                captured_at: row.get(4)?,
            })
        })?;

        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
