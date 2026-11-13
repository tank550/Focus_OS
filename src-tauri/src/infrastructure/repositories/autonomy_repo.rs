use rusqlite::Connection;

use crate::infrastructure::db::DbError;

pub struct AutonomyDecisionRecord<'a> {
    pub id: &'a str,
    pub action_id: &'a str,
    pub occurred_at: i64,
    pub policy: &'a str,
    pub reason: &'a str,
    pub proposed_payload: &'a str,
    pub result_payload: &'a str,
    pub related_task_ids: &'a str,
}

pub struct AutonomyRepo;

impl AutonomyRepo {
    pub fn insert_confirmed_task_from_inbox(
        conn: &Connection,
        decision: &AutonomyDecisionRecord<'_>,
    ) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO autonomy_decisions
                (id, action_id, action_type, occurred_at, trigger, level, policy, status,
                 reason, proposed_payload, result_payload, related_task_ids, created_at, updated_at)
             VALUES (?1, ?2, 'CREATE_TASK_FROM_INBOX', ?3, 'inbox_proposal_approved',
                     'REQUIRE_CONFIRMATION', ?4, 'CONFIRMED', ?5, ?6, ?7, ?8, ?3, ?3)",
            rusqlite::params![
                decision.id,
                decision.action_id,
                decision.occurred_at,
                decision.policy,
                decision.reason,
                decision.proposed_payload,
                decision.result_payload,
                decision.related_task_ids,
            ],
        )?;
        Ok(())
    }
}
