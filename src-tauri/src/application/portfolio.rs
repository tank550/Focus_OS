use serde::{Deserialize, Serialize};
use thiserror::Error;

use focus_domain::ids::EntityId;
use focus_domain::value_objects::UtcMillis;

use crate::application::settings::{self, AiSettings};
use crate::infrastructure::ai::{AiProviderError, NimProvider, PortfolioEvaluation};
use crate::infrastructure::db::DbError;
use crate::infrastructure::github::{GithubClient, GithubError, GithubSourceSummary};
use crate::infrastructure::repositories::portfolio_repo::{
    PortfolioAssessmentRecord, PortfolioRepo,
};
use crate::infrastructure::secrets::{self, SecretError};
use crate::state::AppState;

const NIM_DEFAULT_BASE_URL: &str = "https://integrate.api.nvidia.com";
const MAX_DRAFT_CHARS: usize = 12_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioReport {
    pub id: String,
    pub repo_full_name: String,
    pub source_summary: GithubSourceSummary,
    pub evaluation: PortfolioEvaluation,
    pub draft_markdown: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Error)]
pub enum PortfolioError {
    #[error("AI analysis is disabled in Settings")]
    AiDisabled,
    #[error("Portfolio analysis currently requires the NVIDIA NIM provider")]
    UnsupportedProvider,
    #[error("The repository summary changed after inspection; inspect it again before confirming")]
    SummaryChanged,
    #[error("Choose an NIM model in Settings before analyzing a repository")]
    MissingModel,
    #[error("Save an NVIDIA NIM API key in Settings before analyzing a repository")]
    MissingApiKey,
    #[error("Could not read AI settings: {0}")]
    InvalidSettings(String),
    #[error("Could not serialize portfolio data: {0}")]
    Serialization(String),
    #[error("The portfolio draft cannot be empty")]
    EmptyDraft,
    #[error("The portfolio draft exceeds the {MAX_DRAFT_CHARS}-character limit")]
    DraftTooLong,
    #[error("The portfolio assessment was not found")]
    NotFound,
    #[error(transparent)]
    Provider(#[from] AiProviderError),
    #[error(transparent)]
    Github(#[from] GithubError),
    #[error(transparent)]
    Secrets(#[from] SecretError),
    #[error(transparent)]
    Database(#[from] DbError),
}

pub async fn analyze_repository(
    state: &AppState,
    full_name: &str,
    confirmed_summary: &GithubSourceSummary,
) -> Result<PortfolioReport, PortfolioError> {
    let ai_settings = settings::get_raw(state, settings::KEY_AI)?
        .map(|raw| {
            serde_json::from_str::<AiSettings>(&raw)
                .map_err(|error| PortfolioError::InvalidSettings(error.to_string()))
        })
        .transpose()?
        .unwrap_or_default();
    if !ai_settings.enabled {
        return Err(PortfolioError::AiDisabled);
    }
    if ai_settings.provider != "nim" {
        return Err(PortfolioError::UnsupportedProvider);
    }
    let model = ai_settings
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .ok_or(PortfolioError::MissingModel)?;
    if !secrets::has_nim_api_key()? {
        return Err(PortfolioError::MissingApiKey);
    }

    let github_token = secrets::load_github_token()?;
    let github = GithubClient::new(&github_token)?;
    let source_summary = github.inspect_owned_repository(full_name).await?;
    if &source_summary != confirmed_summary {
        return Err(PortfolioError::SummaryChanged);
    }
    let summary_json = serde_json::to_string(&source_summary)
        .map_err(|error| PortfolioError::Serialization(error.to_string()))?;

    let api_key = secrets::load_nim_api_key()?;
    let provider = NimProvider::new(
        ai_settings
            .base_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .unwrap_or(NIM_DEFAULT_BASE_URL),
        model,
        api_key,
    )?;
    let evaluation = provider.evaluate_portfolio_summary(&summary_json).await?;
    let draft_markdown = render_draft(full_name, &evaluation);
    let serialized_evaluation = serde_json::to_string(&evaluation)
        .map_err(|error| PortfolioError::Serialization(error.to_string()))?;
    let now = UtcMillis::now().0;
    let report = PortfolioReport {
        id: EntityId::new().to_string(),
        repo_full_name: full_name.to_owned(),
        source_summary: source_summary.clone(),
        evaluation,
        draft_markdown: draft_markdown.clone(),
        created_at: now,
        updated_at: now,
    };
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    PortfolioRepo::insert(
        &tx,
        &PortfolioAssessmentRecord {
            id: report.id.clone(),
            repo_full_name: report.repo_full_name.clone(),
            source_summary: summary_json,
            evaluation: serialized_evaluation,
            draft_markdown,
            created_at: now,
            updated_at: now,
        },
    )?;
    tx.commit().map_err(DbError::from)?;
    Ok(report)
}

pub fn list_recent(state: &AppState) -> Result<Vec<PortfolioReport>, PortfolioError> {
    let conn = state.conn()?;
    PortfolioRepo::list_recent(&conn)?
        .into_iter()
        .map(report_from_record)
        .collect()
}

pub fn save_draft(state: &AppState, id: &str, draft_markdown: &str) -> Result<(), PortfolioError> {
    let draft_markdown = draft_markdown.trim();
    if draft_markdown.is_empty() {
        return Err(PortfolioError::EmptyDraft);
    }
    if draft_markdown.chars().count() > MAX_DRAFT_CHARS {
        return Err(PortfolioError::DraftTooLong);
    }

    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    if !PortfolioRepo::update_draft(&tx, id, draft_markdown, UtcMillis::now().0)? {
        return Err(PortfolioError::NotFound);
    }
    tx.commit().map_err(DbError::from)?;
    Ok(())
}

fn report_from_record(
    record: PortfolioAssessmentRecord,
) -> Result<PortfolioReport, PortfolioError> {
    let source_summary = serde_json::from_str(&record.source_summary)
        .map_err(|error| PortfolioError::Serialization(error.to_string()))?;
    let evaluation: PortfolioEvaluation = serde_json::from_str(&record.evaluation)
        .map_err(|error| PortfolioError::Serialization(error.to_string()))?;
    Ok(PortfolioReport {
        id: record.id,
        repo_full_name: record.repo_full_name,
        source_summary,
        evaluation,
        draft_markdown: record.draft_markdown,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

fn render_draft(repo_full_name: &str, evaluation: &PortfolioEvaluation) -> String {
    let mut draft = format!(
        "# {repo_full_name}\n\n{}\n\n## Portfolio readiness\n\n{} / 100 — AI estimate; review before use.\n",
        evaluation.project_summary, evaluation.readiness_score
    );
    draft.push_str("## Strengths\n");
    for strength in &evaluation.strengths {
        draft.push_str(&format!("\n- {strength}"));
    }
    draft.push_str("\n\n## Gaps to address\n");
    for gap in &evaluation.gaps {
        draft.push_str(&format!("\n- {gap}"));
    }
    draft.push_str("\n\n## Resume bullet drafts\n");
    for bullet in &evaluation.resume_bullets {
        draft.push_str(&format!("\n- {bullet}"));
    }
    draft.push_str("\n\n## Verify before using\n");
    for note in &evaluation.verification_notes {
        draft.push_str(&format!("\n- {note}"));
    }
    draft
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_draft_keeps_resume_claims_separate_from_verification_notes() {
        let draft = render_draft(
            "octocat/example",
            &PortfolioEvaluation {
                readiness_score: 72,
                project_summary: "A personal task management application.".into(),
                strengths: vec!["Uses a typed Rust core.".into()],
                gaps: vec!["Deployment evidence is missing.".into()],
                resume_bullets: vec!["Built a desktop task manager with Rust and React.".into()],
                verification_notes: vec!["Confirm user adoption before claiming impact.".into()],
                confidence: 0.8,
            },
        );

        assert!(draft.contains("# octocat/example"));
        assert!(draft.contains("## Strengths"));
        assert!(draft.contains("## Gaps to address"));
        assert!(draft.contains("Built a desktop task manager with Rust and React."));
        assert!(draft.contains("Confirm user adoption before claiming impact."));
        assert!(draft.contains("review before use"));
    }
}
