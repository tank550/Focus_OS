use tauri::State;

use crate::infrastructure::db::{self, DbError};
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub ok: bool,
    pub db: String,
    pub migrations_applied: i64,
    pub database_path: Option<String>,
    pub error: Option<String>,
}

/// Lightweight liveness + DB probe used by the UI status badge.
#[tauri::command]
pub fn health(state: State<'_, AppState>) -> Health {
    let check: Result<i64, DbError> = (|| {
        let mut conn = state.conn()?;
        db::migrations_applied(&mut conn)
    })();

    match check {
        Ok(applied) => Health {
            ok: true,
            db: "ok".into(),
            migrations_applied: applied,
            database_path: state.db_path().to_str().map(str::to_string),
            error: None,
        },
        Err(e) => Health {
            ok: false,
            db: "error".into(),
            migrations_applied: 0,
            database_path: state.db_path().to_str().map(str::to_string),
            error: Some(e.to_string()),
        },
    }
}
