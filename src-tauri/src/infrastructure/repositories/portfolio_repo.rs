use rusqlite::Connection;

use crate::infrastructure::db::DbError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioAssessmentRecord {
    pub id: String,
    pub repo_full_name: String,
    pub source_summary: String,
    pub evaluation: String,
    pub draft_markdown: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct PortfolioRepo;

impl PortfolioRepo {
    pub fn insert(
        conn: &Connection,
        assessment: &PortfolioAssessmentRecord,
    ) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO portfolio_assessments
                (id, repo_full_name, source_summary, evaluation, draft_markdown,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                assessment.id,
                assessment.repo_full_name,
                assessment.source_summary,
                assessment.evaluation,
                assessment.draft_markdown,
                assessment.created_at,
                assessment.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_recent(conn: &Connection) -> Result<Vec<PortfolioAssessmentRecord>, DbError> {
        let mut statement = conn.prepare(
            "SELECT id, repo_full_name, source_summary, evaluation, draft_markdown,
                    created_at, updated_at
             FROM portfolio_assessments
             ORDER BY created_at DESC, id DESC
             LIMIT 100",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(PortfolioAssessmentRecord {
                id: row.get(0)?,
                repo_full_name: row.get(1)?,
                source_summary: row.get(2)?,
                evaluation: row.get(3)?,
                draft_markdown: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn update_draft(
        conn: &Connection,
        id: &str,
        draft_markdown: &str,
        updated_at: i64,
    ) -> Result<bool, DbError> {
        let changed = conn.execute(
            "UPDATE portfolio_assessments
             SET draft_markdown = ?2, updated_at = ?3
             WHERE id = ?1",
            rusqlite::params![id, draft_markdown, updated_at],
        )?;
        Ok(changed == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assessment_and_edited_draft_round_trip() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0003_portfolio_assessments.sql"
        )))
        .unwrap();
        let assessment = PortfolioAssessmentRecord {
            id: "assessment-1".into(),
            repo_full_name: "owner/repository".into(),
            source_summary: r#"{"fullName":"owner/repository"}"#.into(),
            evaluation: r#"{"readinessScore":70}"#.into(),
            draft_markdown: "# First draft".into(),
            created_at: 10,
            updated_at: 10,
        };

        PortfolioRepo::insert(&conn, &assessment).unwrap();
        let listed = PortfolioRepo::list_recent(&conn).unwrap();
        assert_eq!(listed, vec![assessment]);

        assert!(PortfolioRepo::update_draft(&conn, "assessment-1", "# Edited", 20).unwrap());
        let listed = PortfolioRepo::list_recent(&conn).unwrap();
        assert_eq!(listed[0].draft_markdown, "# Edited");
        assert_eq!(listed[0].updated_at, 20);
        assert!(!PortfolioRepo::update_draft(&conn, "missing", "# Draft", 30).unwrap());
    }
}
