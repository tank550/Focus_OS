//! Compatibility entry point for the canonical migration runner in `db`.

use rusqlite::Connection;

pub use crate::infrastructure::db::{DbError as MigrationError, Migration, MIGRATIONS};

pub fn apply_migrations(conn: &mut Connection) -> Result<usize, MigrationError> {
    crate::infrastructure::db::apply_migrations(conn)
}
