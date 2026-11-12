use serde::Serialize;
use thiserror::Error;

use focus_domain::ids::EntityId;
use focus_domain::value_objects::UtcMillis;

use crate::application::inbox::InboxError;
use crate::application::settings::{self, AiSettings};
use crate::infrastructure::ai::{
    AiAnalysisResult, AiProviderError, AiProviderKind, MockProvider, NimProvider,
};
use crate::infrastructure::db::DbError;
use crate::infrastructure::repositories::inbox_repo::InboxRepo;
use crate::infrastructure::secrets::{self, SecretError};
use crate::state::AppState;

const NIM_DEFAULT_BASE_URL: &str = "https://integrate.api.nvidia.com";

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("AI analysis is disabled in Settings")]
    Disabled,
    #[error("The inbox note was not found or is archived")]
    NoteNotFound,
    #[error("The note changed while it was being analyzed; run the analysis again")]
    NoteChanged,
    #[error("Choose an NIM model in Settings before analyzing a note")]
    MissingModel,
    #[error("Save an NVIDIA NIM API key in Settings before analyzing a note")]
    MissingApiKey,
    #[error("Unsupported AI provider: {0}")]
    UnsupportedProvider(String),
    #[error("Could not read AI settings: {0}")]
    InvalidSettings(String),
    #[error("Could not store the analysis: {0}")]
    Serialization(String),
    #[error(transparent)]
    Provider(#[from] AiProviderError),
    #[error(transparent)]
    Secrets(#[from] SecretError),
    #[error(transparent)]
    Inbox(#[from] InboxError),
    #[error(transparent)]
    Database(#[from] DbError),
}

#[derive(Serialize)]
struct StoredAnalysis<'a> {
    provider: AiProviderKind,
    analysis: &'a crate::infrastructure::ai::BrainDumpAnalysis,
}

pub async fn analyze_inbox_note(
    state: &AppState,
    inbox_item_id: &str,
) -> Result<AiAnalysisResult, AgentError> {
    let record = {
        let conn = state.conn()?;
        InboxRepo::get(&conn, inbox_item_id)?.ok_or(AgentError::NoteNotFound)?
    };

    let ai_settings = settings::get_raw(state, settings::KEY_AI)?
        .map(|raw| {
            serde_json::from_str::<AiSettings>(&raw)
                .map_err(|error| AgentError::InvalidSettings(error.to_string()))
        })
        .transpose()?
        .unwrap_or_default();
    if !ai_settings.enabled {
        return Err(AgentError::Disabled);
    }

    let result = match ai_settings.provider.as_str() {
        "mock" => AiAnalysisResult {
            provider: AiProviderKind::Mock,
            analysis: MockProvider::analyze(),
        },
        "nim" => {
            if ai_settings
                .model
                .as_deref()
                .map(str::trim)
                .filter(|model| !model.is_empty())
                .is_none()
            {
                return Err(AgentError::MissingModel);
            }
            if !secrets::has_nim_api_key()? {
                return Err(AgentError::MissingApiKey);
            }
            let api_key = secrets::load_nim_api_key()?;
            let provider = NimProvider::new(
                ai_settings
                    .base_url
                    .as_deref()
                    .filter(|url| !url.trim().is_empty())
                    .unwrap_or(NIM_DEFAULT_BASE_URL),
                ai_settings
                    .model
                    .as_deref()
                    .ok_or(AgentError::MissingModel)?,
                api_key,
            )?;
            AiAnalysisResult {
                provider: AiProviderKind::Nim,
                analysis: provider.analyze(&record.raw_text).await?,
            }
        }
        provider => return Err(AgentError::UnsupportedProvider(provider.to_owned())),
    };

    let serialized = serde_json::to_string(&StoredAnalysis {
        provider: result.provider,
        analysis: &result.analysis,
    })
    .map_err(|error| AgentError::Serialization(error.to_string()))?;
    let analyzed_at = UtcMillis::now().0;
    let mut conn = state.conn()?;
    let tx = conn.transaction().map_err(DbError::from)?;
    if !InboxRepo::record_analysis(
        &tx,
        inbox_item_id,
        &record.raw_text,
        record.updated_at,
        &EntityId::new().to_string(),
        &serialized,
        result.analysis.kind.as_str(),
        result.analysis.confidence,
        analyzed_at,
    )? {
        return Err(AgentError::NoteChanged);
    }
    tx.commit().map_err(DbError::from)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::inbox;
    use crate::infrastructure::ai::Actionability;

    fn test_state() -> (std::path::PathBuf, AppState) {
        let directory = std::env::temp_dir().join(format!("focus-os-agent-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        (directory, AppState::new(pool, database_path))
    }

    #[tokio::test]
    async fn mock_analysis_is_saved_as_a_proposal_without_mutating_the_note() {
        let (directory, state) = test_state();
        settings::set_raw(
            &state,
            settings::KEY_AI,
            r#"{"enabled":true,"provider":"mock","baseUrl":null,"model":null,"apiKeyStored":false}"#,
        )
        .unwrap();
        let note = inbox::capture(&state, "A thought to sort later").unwrap();

        let result = analyze_inbox_note(&state, &note.id).await.unwrap();

        assert_eq!(result.provider, AiProviderKind::Mock);
        assert_eq!(result.analysis.actionability, Actionability::NotActionable);
        let updated_note = inbox::list(&state).unwrap().remove(0);
        assert_eq!(updated_note.raw_text, note.raw_text);
        assert_eq!(updated_note.kind, "NOTE");
        assert_eq!(updated_note.status, "PROPOSED");
        assert_eq!(updated_note.analysis, Some(result));

        let conn = state.conn().unwrap();
        let proposal_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM classification_proposals
                 WHERE inbox_item_id = ?1 AND status = 'PENDING'",
                [&note.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(proposal_count, 1);
        drop(conn);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
