use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::infrastructure::db::DbError;
use crate::infrastructure::repositories::settings_repo::SettingsRepo;
use crate::state::AppState;

pub const KEY_AI: &str = "ai";
pub const KEY_SCHEDULE: &str = "schedule";
pub const KEY_APPEARANCE: &str = "appearance";

/// Typed views over the JSON-blob settings. Values mirror
/// `src/shared/contracts.ts`; keep them byte-identical.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiSettings {
    pub enabled: bool,
    pub provider: String, // "nim" | "mock" (Phase 4 wires real providers)
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key_stored: bool,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: "mock".into(),
            base_url: None,
            model: None,
            api_key_stored: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleSettings {
    pub working_hours_start: String, // 'HH:MM' 24h
    pub working_hours_end: String,
    pub max_daily_minutes: u32,
    pub reserve_ratio: f64, // 0..1 protected unfilled capacity
}

impl Default for ScheduleSettings {
    fn default() -> Self {
        Self {
            working_hours_start: "09:00".into(),
            working_hours_end: "18:00".into(),
            max_daily_minutes: 480,
            reserve_ratio: 0.15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceSettings {
    pub theme: String, // dark-first; extended in Phase 8
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Service functions (the transaction boundary for settings mutations)
// ---------------------------------------------------------------------------

pub fn get_raw(state: &AppState, key: &str) -> Result<Option<String>, DbError> {
    let mut conn = state.conn()?;
    SettingsRepo::get(&mut conn, key)
}

pub fn set_raw(state: &AppState, key: &str, value: &str) -> Result<(), DbError> {
    let mut conn = state.conn()?;
    let tx = conn.transaction()?;
    SettingsRepo::set(&tx, key, value)?;
    tx.commit()?;
    Ok(())
}

pub fn all_raw(state: &AppState) -> Result<BTreeMap<String, String>, DbError> {
    let mut conn = state.conn()?;
    SettingsRepo::all(&mut conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_settings_defaults_serialize_camel_case() {
        let ai = serde_json::to_value(AiSettings::default()).unwrap();
        assert_eq!(ai["provider"], "mock");
        assert_eq!(ai["apiKeyStored"], false);
        assert!(ai.get("api_key_stored").is_none(), "must be camelCase");

        let sched = serde_json::to_value(ScheduleSettings::default()).unwrap();
        assert_eq!(sched["workingHoursStart"], "09:00");
        assert_eq!(sched["maxDailyMinutes"], 480);

        let app = serde_json::to_value(AppearanceSettings::default()).unwrap();
        assert_eq!(app["theme"], "dark");
    }
}