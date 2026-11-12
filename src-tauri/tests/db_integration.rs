//! Integration tests for the SQLite layer — migrations, constraints, and
//! repository correctness against a real database file (never the app DB).

use std::path::PathBuf;

use focus_os_lib::infrastructure::db;
use focus_os_lib::infrastructure::repositories::settings_repo::SettingsRepo;

/// Fresh, process-unique temp DB per test.
fn temp_database(test: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "focus-os-test-{}-{}",
        test,
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("test.db");
    let _ = std::fs::remove_file(&path); // start clean
    (dir, path)
}

#[test]
fn migrations_apply_cleanly_and_are_idempotent() {
    let (dir, path) = temp_database("migrations");

    let pool = db::open(&path).expect("open db");
    let mut conn = pool.get().expect("lease conn");
    assert_eq!(db::migrations_applied(&mut conn).unwrap(), 1);
    assert_eq!(db::count_tables(&mut conn).unwrap(), 21);

    // Second open against the same file must be a migration no-op.
    drop(conn);
    drop(pool);
    let pool2 = db::open(&path).expect("re-open db");
    let mut conn2 = pool2.get().expect("lease conn 2");
    assert_eq!(db::migrations_applied(&mut conn2).unwrap(), 1);

    drop(conn2);
    drop(pool2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn schema_expected_tables_exist() {
    let (dir, path) = temp_database("tables");
    let pool = db::open(&path).expect("open db");
    let mut conn = pool.get().expect("lease conn");

    let expected = [
        "goals",
        "milestones",
        "projects",
        "tasks",
        "task_dependencies",
        "inbox_items",
        "daily_plans",
        "scheduled_blocks",
        "flexible_queue_items",
        "task_sessions",
        "daily_reviews",
        "quarterly_reviews",
        "schedule_events",
        "autonomy_decisions",
        "approval_requests",
        "ai_planning_history",
        "notifications",
        "app_settings",
        "scheduler_state",
    ];
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .unwrap();
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    for table in expected {
        assert!(
            names.iter().any(|n| n == table),
            "expected table {table} to exist"
        );
    }

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn settings_roundtrip_and_upsert() {
    let (dir, path) = temp_database("settings");
    let pool = db::open(&path).expect("open db");
    let mut conn = pool.get().expect("lease conn");

    assert!(SettingsRepo::get(&mut conn, "ai").unwrap().is_none());

    SettingsRepo::set(&mut conn, "ai", r#"{"enabled":true}"#).unwrap();
    assert_eq!(
        SettingsRepo::get(&mut conn, "ai").unwrap().unwrap(),
        r#"{"enabled":true}"#
    );

    // Upsert must overwrite, not duplicate.
    SettingsRepo::set(&mut conn, "ai", r#"{"enabled":false}"#).unwrap();
    let all = SettingsRepo::all(&mut conn).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all["ai"], r#"{"enabled":false}"#);

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn wal_and_foreign_keys_are_active_on_pool_connections() {
    let (dir, path) = temp_database("wal");
    let pool = db::open(&path).expect("open db");
    let conn = pool.get().expect("lease conn");

    let mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode.to_lowercase(), "wal");

    let fk: i64 = conn
        .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1, "foreign_keys must be ON per connection");

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn foreign_key_violations_are_typed_errors() {
    let (dir, path) = temp_database("fk");
    let pool = db::open(&path).expect("open db");
    let mut conn = pool.get().expect("lease conn");

    // project_id references a non-existent project — must be rejected.
    let res = conn.execute(
        "INSERT INTO tasks (id, project_id, title, status, estimated_minutes, created_at, updated_at)
         VALUES ('0190dc62-0000-7000-8000-000000000001', 'no-such-project',
                 'dead-end task', 'TODO', 30, 1, 1)",
        [],
    );
    assert!(res.is_err(), "FK violation must surface as an error");
    assert!(
        res.err()
            .unwrap()
            .to_string()
            .to_lowercase()
            .contains("foreign"),
        "error should mention foreign key"
    );

    // No row may have been inserted.
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_constraints_reject_garbage_status() {
    let (dir, path) = temp_database("check");
    let pool = db::open(&path).expect("open db");
    let mut conn = pool.get().expect("lease conn");

    let res = conn.execute(
        "INSERT INTO tasks (id, title, status, estimated_minutes, created_at, updated_at)
         VALUES ('0190dc62-0000-7000-8000-000000000002', 'oracle', 'BOGUS', 30, 1, 1)",
        [],
    );
    assert!(res.is_err(), "CHECK constraint must reject BOGUS status");

    drop(conn);
    drop(pool);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn transaction_rollback_leaves_no_partial_rows() {
    let (dir, path) = temp_database("tx");
    let pool = db::open(&path).expect("open db");
    let mut conn = pool.get().expect("lease conn");

    {
        let tx = conn.transaction().unwrap();
        tx.execute(
            "INSERT INTO goals (id, title, priority, status, progress, created_at, updated_at)
             VALUES ('0190dc62-0000-7000-8000-000000000003', 'rollback me', 50, 'ACTIVE', 0.0, 1, 1)",
            [],
        )
        .unwrap();
        // scope ends -> transaction dropped without commit
    }
    drop(conn);

    let mut conn2 = pool.get().unwrap();
    let count: i64 = conn2
        .query_row("SELECT COUNT(*) FROM goals", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0, "aborted transaction must not persist any rows");

    drop(conn2);
    drop(pool);
    let _ = std::fs::remove_dir_all(&dir);
}