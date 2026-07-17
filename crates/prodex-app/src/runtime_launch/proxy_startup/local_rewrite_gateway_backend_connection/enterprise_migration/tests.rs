//! Enterprise migration regression tests.

use super::{
    runtime_gateway_sqlite_create_current_schema_for_tests,
    runtime_gateway_sqlite_migrate_enterprise_state, runtime_gateway_sqlite_open,
};
use prodex_storage_sqlite::{
    REQUIRED_SQLITE_SCHEMA_VERSION, SqliteRuntimeMode, plan_sqlite_migrations,
};
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("prodex-enterprise-migration-{name}-{stamp}"))
}

#[test]
fn sqlite_enterprise_migrations_are_versioned_and_idempotent() {
    let root = temp_dir("versioned");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");

    let first = runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap();
    let second = runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap();

    assert_eq!(
        first,
        plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator)
            .unwrap()
            .migrations
            .len()
    );
    assert_eq!(second, 0);
    let conn = Connection::open(&path).unwrap();
    let (count, max_version): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), MAX(version) FROM prodex_enterprise_schema_migrations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(count, i64::from(REQUIRED_SQLITE_SCHEMA_VERSION.0));
    assert_eq!(max_version, i64::from(REQUIRED_SQLITE_SCHEMA_VERSION.0));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_enterprise_migrator_bootstraps_legacy_current_schema() {
    let root = temp_dir("legacy");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    let conn = Connection::open(&path).unwrap();
    let plan = plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator).unwrap();
    for migration in &plan.migrations {
        conn.execute_batch(migration.sql).unwrap();
    }
    drop(conn);

    assert_eq!(
        runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap(),
        0
    );
    let conn = Connection::open(&path).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prodex_enterprise_schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, plan.migrations.len() as i64);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_enterprise_migration_failure_rolls_back_its_ddl_and_ledger_row() {
    let root = temp_dir("rollback");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE prodex_enterprise_schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            checksum TEXT NOT NULL,
            applied_at_epoch INTEGER NOT NULL
        );
        CREATE TABLE prodex_governance_sessions (
            tenant_id TEXT,
            principal_id TEXT,
            absolute_expires_at_unix_ms INTEGER,
            idle_expires_at_unix_ms INTEGER,
            session_id_hash TEXT,
            last_seen_at_unix_ms INTEGER,
            registry_revision_id TEXT,
            provider_descriptor_revision INTEGER
        );",
    )
    .unwrap();
    drop(conn);

    let error = runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("006_governance_session_provider_revisions")
    );
    let conn = Connection::open(&path).unwrap();
    let max_version: i64 = conn
        .query_row(
            "SELECT MAX(version) FROM prodex_enterprise_schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let legacy_column_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('prodex_governance_sessions') WHERE name = 'registry_revision_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let renamed_column_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('prodex_governance_sessions') WHERE name = 'provider_registry_revision'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(max_version, 5);
    assert_eq!(legacy_column_count, 1);
    assert_eq!(renamed_column_count, 0);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_enterprise_migrator_rejects_checksum_drift() {
    let root = temp_dir("checksum");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE prodex_enterprise_schema_migrations SET checksum = 'tampered' WHERE version = 4",
        [],
    )
    .unwrap();
    drop(conn);

    let error = runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap_err();
    assert!(error.to_string().contains("checksum does not match"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_open_rejects_old_enterprise_schema_version() {
    let root = temp_dir("old-version");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "DELETE FROM prodex_enterprise_schema_migrations WHERE version = ?1",
        [i64::from(REQUIRED_SQLITE_SCHEMA_VERSION.0)],
    )
    .unwrap();
    drop(conn);

    let error = runtime_gateway_sqlite_open(&path).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("SQLite schema version is too old")
    );

    std::fs::remove_dir_all(root).unwrap();
}
