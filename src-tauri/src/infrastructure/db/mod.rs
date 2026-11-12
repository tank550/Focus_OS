//! SQLite persistence — pool, WAL pragmas, and the versioned migration runner.
//!
//! Conventions enforced here match `docs/SQLITE_SCHEMA.sql`:
//!   * foreign_keys always ON (per connection),
//!   * WAL journaling + busy_timeout (single-writer friendliness),
//!   * each migration applies inside one transaction and is recorded in
//!     `schema_migrations`.

use std::collections::HashSet;
use std::path::Path;

use focus_domain::value_objects::UtcMillis;
use r2d2::{CustomizeConnection, Pool};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use tauri::Manager;
use thiserror::Error;

pub const DB_FILENAME: &str = "focus-os.db";

/// Connection pool shared across commands (state-owned).
pub type DbPool = Pool<SqliteConnectionManager>;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("app data directory unavailable: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("connection pool error: {0}")]
    Pool(#[from] r2d2::Error),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration {version} failed: {source}")]
    Migration {
        version: i64,
        #[source]
        source: rusqlite::Error,
    },
}

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

/// Ordered, immutable migration set. Add new versions by appending — never edit
/// an applied migration. The embedded file is a byte-identical copy of
/// `docs/SQLITE_SCHEMA.sql` (hashes are pinned in the phase-1 notes).
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0001_init.sql"
        )),
    },
    Migration {
        version: 2,
        name: "goal_sidecar",
        sql: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0002_goal_sidecar.sql"
        )),
    },
];

/// Applies WAL + safety pragmas to every freshly-created connection.
#[derive(Debug)]
struct ConnectionTuning;

impl CustomizeConnection<Connection, rusqlite::Error> for ConnectionTuning {
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        Ok(())
    }
}

/// Open (creating if needed) the database at `path`, apply migrations, and
/// return a ready pool.
pub fn open(path: &Path) -> Result<DbPool, DbError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let manager = SqliteConnectionManager::file(path);
    let pool = Pool::builder()
        .max_size(4)
        .connection_customizer(Box::new(ConnectionTuning))
        .build(manager)?;

    let mut conn = pool.get()?;
    apply_migrations(&mut conn)?;
    Ok(pool)
}

/// Resolve the app-data directory and open the canonical database file.
pub fn init(app: &tauri::AppHandle) -> Result<DbPool, DbError> {
    let dir = app.path().app_data_dir()?;
    open(&dir.join(DB_FILENAME))
}

/// Apply pending migrations inside individual transactions. Returns how many
/// migrations were newly applied (idempotent on repeated calls).
pub fn apply_migrations(conn: &mut Connection) -> Result<usize, DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let applied: HashSet<i64> = conn
        .prepare("SELECT version FROM schema_migrations")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<HashSet<_>, _>>()?;

    let mut applied_count = 0usize;
    for migration in MIGRATIONS {
        if applied.contains(&migration.version) {
            continue;
        }
        let tx = conn.transaction()?;
        if let Err(source) = tx.execute_batch(migration.sql) {
            return Err(DbError::Migration {
                version: migration.version,
                source,
            });
        }
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![migration.version, migration.name, UtcMillis::now().0],
        )?;
        tx.commit()?;
        applied_count += 1;
    }
    Ok(applied_count)
}

/// Number of recorded migrations (health probe + tests).
pub fn migrations_applied(conn: &mut Connection) -> Result<i64, DbError> {
    Ok(conn.query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))?)
}

/// Number of application tables (excluding sqlite internals + bookkeeping).
pub fn count_tables(conn: &mut Connection) -> Result<usize, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'table'
           AND name NOT LIKE 'sqlite_%'
           AND name != 'schema_migrations'",
        [],
        |r| r.get(0),
    )?;
    Ok(count as usize)
}
