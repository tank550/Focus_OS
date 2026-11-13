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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingProposalRecord {
    pub id: String,
    pub raw_text: String,
    pub proposal: String,
}

pub struct AnalysisProposal<'a> {
    pub inbox_item_id: &'a str,
    pub expected_text: &'a str,
    pub expected_updated_at: i64,
    pub proposal_id: &'a str,
    pub proposal_json: &'a str,
    pub kind: &'a str,
    pub confidence: f64,
    pub analyzed_at: i64,
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
        proposal: &AnalysisProposal<'_>,
    ) -> Result<bool, DbError> {
        let changed = conn.execute(
            "UPDATE inbox_items
             SET kind = ?4, status = 'PROPOSED', classification = ?5,
                 classification_proposed_at = ?6, updated_at = ?6
             WHERE id = ?1 AND raw_text = ?2 AND updated_at = ?3
               AND archived = 0 AND status IN ('RAW','CLASSIFIED','PROPOSED')",
            rusqlite::params![
                proposal.inbox_item_id,
                proposal.expected_text,
                proposal.expected_updated_at,
                proposal.kind,
                proposal.proposal_json,
                proposal.analyzed_at
            ],
        )?;
        if changed != 1 {
            return Ok(false);
        }

        conn.execute(
            "UPDATE classification_proposals
             SET status = 'SUPERSEDED', decided_at = ?2
             WHERE inbox_item_id = ?1 AND status = 'PENDING'",
            rusqlite::params![proposal.inbox_item_id, proposal.analyzed_at],
        )?;
        conn.execute(
            "INSERT INTO classification_proposals
                (id, inbox_item_id, proposal, status, confidence, created_at)
             VALUES (?1, ?2, ?3, 'PENDING', ?4, ?5)",
            rusqlite::params![
                proposal.proposal_id,
                proposal.inbox_item_id,
                proposal.proposal_json,
                proposal.confidence,
                proposal.analyzed_at
            ],
        )?;
        Ok(true)
    }

    pub fn pending_proposal(
        conn: &Connection,
        inbox_item_id: &str,
    ) -> Result<Option<PendingProposalRecord>, DbError> {
        Ok(conn
            .query_row(
                "SELECT cp.id, ii.raw_text, cp.proposal
                 FROM classification_proposals cp
                 JOIN inbox_items ii ON ii.id = cp.inbox_item_id
                 WHERE cp.inbox_item_id = ?1 AND cp.status = 'PENDING'
                   AND ii.status = 'PROPOSED' AND ii.archived = 0
                 ORDER BY cp.created_at DESC, cp.id DESC LIMIT 1",
                [inbox_item_id],
                |row| {
                    Ok(PendingProposalRecord {
                        id: row.get(0)?,
                        raw_text: row.get(1)?,
                        proposal: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn approve_proposal(
        conn: &Connection,
        inbox_item_id: &str,
        proposal_id: &str,
        approved_at: i64,
    ) -> Result<bool, DbError> {
        let changed = conn.execute(
            "UPDATE classification_proposals
             SET status = 'APPROVED', decided_at = ?3, decided_by = 'user'
             WHERE id = ?1 AND inbox_item_id = ?2 AND status = 'PENDING'",
            rusqlite::params![proposal_id, inbox_item_id, approved_at],
        )?;
        if changed != 1 {
            return Ok(false);
        }

        let changed = conn.execute(
            "UPDATE inbox_items
             SET status = 'CONVERTED', classification_approved_at = ?2, updated_at = ?2
             WHERE id = ?1 AND status = 'PROPOSED' AND archived = 0",
            rusqlite::params![inbox_item_id, approved_at],
        )?;
        Ok(changed == 1)
    }

    pub fn applied_task_action(
        conn: &Connection,
        inbox_item_id: &str,
    ) -> Result<Option<String>, DbError> {
        Ok(conn
            .query_row(
                "SELECT ad.result_payload
                 FROM classification_proposals cp
                 JOIN autonomy_decisions ad ON ad.action_id = cp.id
                 JOIN inbox_items ii ON ii.id = cp.inbox_item_id
                 WHERE cp.inbox_item_id = ?1 AND cp.status = 'APPROVED'
                   AND ad.action_type = 'CREATE_TASK_FROM_INBOX'
                   AND ii.status = 'CONVERTED' AND ii.archived = 0
                 ORDER BY cp.decided_at DESC, cp.id DESC LIMIT 1",
                [inbox_item_id],
                |row| row.get(0),
            )
            .optional()?)
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
