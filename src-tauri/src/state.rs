//! Shared application state injected into every Tauri command.

use std::path::{Path, PathBuf};

use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;

use crate::infrastructure::db::{DbError, DbPool};

pub struct AppState {
    pool: DbPool,
    db_path: PathBuf,
}

impl AppState {
    pub fn new(pool: DbPool, db_path: PathBuf) -> Self {
        Self { pool, db_path }
    }

    /// Lease a connection from the pool. Every repository call goes through
    /// this so pool sizing/PRAGMAs are centralized.
    pub fn conn(&self) -> Result<PooledConnection<SqliteConnectionManager>, DbError> {
        Ok(self.pool.get()?)
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}