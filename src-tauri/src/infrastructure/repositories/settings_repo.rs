use std::collections::BTreeMap;

use focus_domain::value_objects::UtcMillis;
use rusqlite::{Connection, OptionalExtension};

use crate::infrastructure::db::DbError;

/// CRUD for the `app_settings` key/value JSON store.
pub struct SettingsRepo;

impl SettingsRepo {
    pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, DbError> {
        Ok(conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Upsert; `updated_at` always refreshes.
    pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                 value = excluded.value,
                 updated_at = excluded.updated_at",
            rusqlite::params![key, value, UtcMillis::now().0],
        )?;
        Ok(())
    }

    pub fn all(conn: &Connection) -> Result<BTreeMap<String, String>, DbError> {
        let mut stmt = conn.prepare("SELECT key, value FROM app_settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = BTreeMap::new();
        for row in rows {
            let (k, v) = row?;
            out.insert(k, v);
        }
        Ok(out)
    }
}
