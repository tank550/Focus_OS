use rusqlite::{Connection, OptionalExtension};

use crate::infrastructure::db::DbError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxItemRecord {
    pub id: String,
    pub raw_text: String,
    pub kind: String,
    pub status: String,
    pub captured_at: i64,
    pub updated_at: i64,
    pub classification: Option<String>,
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
            "SELECT id, raw_text, kind, status, captured_at, updated_at, classification
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
                updated_at: row.get(5)?,
                classification: row.get(6)?,
            })
        })?;

        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get(conn: &Connection, id: &str) -> Result<Option<InboxItemRecord>, DbError> {
        Ok(conn
            .query_row(
                "SELECT id, raw_text, kind, status, captured_at, updated_at, classification
                 FROM inbox_items
                 WHERE id = ?1 AND archived = 0
                   AND status IN ('RAW','CLASSIFIED','PROPOSED')",
                [id],
                |row| {
                    Ok(InboxItemRecord {
                        id: row.get(0)?,
                        raw_text: row.get(1)?,
                        kind: row.get(2)?,
                        status: row.get(3)?,
                        captured_at: row.get(4)?,
                        updated_at: row.get(5)?,
                        classification: row.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn update(
        conn: &Connection,
        id: &str,
        raw_text: &str,
        updated_at: i64,
    ) -> Result<bool, DbError> {
        let changed = conn.execute(
            "UPDATE inbox_items
             SET raw_text = ?2, kind = 'UNKNOWN', status = 'RAW',
                 classification = NULL, classification_proposed_at = NULL,
                 classification_approved_at = NULL, project_id = NULL, goal_id = NULL,
                 updated_at = ?3
             WHERE id = ?1 AND archived = 0 AND status != 'ARCHIVED'",
            rusqlite::params![id, raw_text, updated_at],
        )?;
        if changed == 1 {
            conn.execute(
                "UPDATE classification_proposals
                 SET status = 'SUPERSEDED', decided_at = ?2
                 WHERE inbox_item_id = ?1 AND status = 'PENDING'",
                rusqlite::params![id, updated_at],
            )?;
        }
        Ok(changed == 1)
    }

    pub fn record_analysis(
        conn: &Connection,
        inbox_item_id: &str,
        expected_text: &str,
        expected_updated_at: i64,
        proposal_id: &str,
        proposal_json: &str,
        kind: &str,
        confidence: f64,
        analyzed_at: i64,
    ) -> Result<bool, DbError> {
        let changed = conn.execute(
            "UPDATE inbox_items
             SET kind = ?4, status = 'PROPOSED', classification = ?5,
                 classification_proposed_at = ?6, updated_at = ?6
             WHERE id = ?1 AND raw_text = ?2 AND updated_at = ?3
               AND archived = 0 AND status IN ('RAW','CLASSIFIED','PROPOSED')",
            rusqlite::params![
                inbox_item_id,
                expected_text,
                expected_updated_at,
                kind,
                proposal_json,
                analyzed_at
            ],
        )?;
        if changed != 1 {
            return Ok(false);
        }

        conn.execute(
            "UPDATE classification_proposals
             SET status = 'SUPERSEDED', decided_at = ?2
             WHERE inbox_item_id = ?1 AND status = 'PENDING'",
            rusqlite::params![inbox_item_id, analyzed_at],
        )?;
        conn.execute(
            "INSERT INTO classification_proposals
                (id, inbox_item_id, proposal, status, confidence, created_at)
             VALUES (?1, ?2, ?3, 'PENDING', ?4, ?5)",
            rusqlite::params![
                proposal_id,
                inbox_item_id,
                proposal_json,
                confidence,
                analyzed_at
            ],
        )?;
        Ok(true)
    }

    pub fn archive(conn: &Connection, id: &str, updated_at: i64) -> Result<bool, DbError> {
        let changed = conn.execute(
            "UPDATE inbox_items
             SET archived = 1, status = 'ARCHIVED', updated_at = ?2
             WHERE id = ?1 AND archived = 0 AND status != 'ARCHIVED'",
            rusqlite::params![id, updated_at],
        )?;
        Ok(changed == 1)
    }
}
