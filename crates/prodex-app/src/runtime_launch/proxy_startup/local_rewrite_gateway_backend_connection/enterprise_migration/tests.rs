//! Enterprise migration regression tests.

use super::enterprise_migration::infer_legacy_postgres_version;
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

    drop(conn);
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

    drop(conn);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_enterprise_migrator_rejects_partial_siem_outbox_leasing_shape() {
    let root = temp_dir("partial-siem-leasing");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    let conn = Connection::open(&path).unwrap();
    let plan = plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator).unwrap();
    for migration in plan.migrations.iter().take(12) {
        conn.execute_batch(migration.sql).unwrap();
    }
    conn.execute(
        "ALTER TABLE prodex_siem_outbox ADD COLUMN claim_token TEXT",
        [],
    )
    .unwrap();
    drop(conn);

    let error = runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap_err();
    assert!(error.to_string().contains("cannot infer"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_enterprise_migrator_rejects_unrecognized_ledgerless_future_shape() {
    let root = temp_dir("future-siem-shape");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    let conn = Connection::open(&path).unwrap();
    let plan = plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator).unwrap();
    for migration in plan.migrations.iter().take(12) {
        conn.execute_batch(migration.sql).unwrap();
    }
    conn.execute(
        "ALTER TABLE prodex_siem_outbox ADD COLUMN future_delivery_state TEXT",
        [],
    )
    .unwrap();
    drop(conn);

    let error = runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap_err();
    assert!(error.to_string().contains("cannot infer"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_enterprise_migrator_rejects_same_name_siem_marker_with_wrong_owner() {
    let root = temp_dir("wrong-owner-siem-marker");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    let conn = Connection::open(&path).unwrap();
    let plan = plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator).unwrap();
    for migration in plan.migrations.iter().take(13) {
        conn.execute_batch(migration.sql).unwrap();
    }
    conn.execute_batch(
        "DROP TRIGGER prodex_siem_outbox_claim_pair_insert;
         CREATE TABLE siem_outbox_decoy (id INTEGER);
         CREATE TRIGGER prodex_siem_outbox_claim_pair_insert
         BEFORE INSERT ON siem_outbox_decoy
         BEGIN
             SELECT 1;
         END;",
    )
    .unwrap();
    drop(conn);

    let error = runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap_err();
    assert!(error.to_string().contains("wrong owner"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_enterprise_migrator_rejects_wrong_siem_outbox_column_and_check_shapes() {
    for (name, event_id_type, check) in [
        ("wrong-column", "BLOB", "CHECK (attempt_count >= 0)"),
        ("wrong-check", "TEXT", "CHECK (attempt_count > 0)"),
    ] {
        let root = temp_dir(name);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("state.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(&format!(
            "CREATE TABLE prodex_tenants (tenant_id TEXT PRIMARY KEY);
             CREATE TABLE prodex_siem_outbox (
                 tenant_id TEXT NOT NULL REFERENCES prodex_tenants(tenant_id),
                 event_id {event_id_type} NOT NULL,
                 audit_event_id TEXT NOT NULL,
                 event_envelope TEXT NOT NULL,
                 attempt_count INTEGER NOT NULL DEFAULT 0,
                 next_attempt_at_unix_ms INTEGER NOT NULL,
                 created_at_unix_ms INTEGER NOT NULL,
                 delivered_at_unix_ms INTEGER,
                 PRIMARY KEY (tenant_id, event_id),
                 UNIQUE (tenant_id, audit_event_id),
                 {check}
             );"
        ))
        .unwrap();
        drop(conn);

        let error = runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap_err();
        assert!(
            error.to_string().contains("cannot infer"),
            "{name}: {error}"
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn sqlite_enterprise_migrator_rejects_wrong_siem_outbox_index_order() {
    let root = temp_dir("wrong-index-order");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    let conn = Connection::open(&path).unwrap();
    let plan = plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator).unwrap();
    for migration in plan.migrations.iter().take(13) {
        conn.execute_batch(migration.sql).unwrap();
    }
    conn.execute_batch(
        "DROP INDEX prodex_siem_outbox_due_claim_idx;
         CREATE INDEX prodex_siem_outbox_due_claim_idx
         ON prodex_siem_outbox (
             next_attempt_at_unix_ms, delivered_at_unix_ms, event_id,
             claim_expires_at_unix_ms
         );",
    )
    .unwrap();
    drop(conn);

    let error = runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap_err();
    assert!(error.to_string().contains("index columns"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_enterprise_migrator_rejects_noop_siem_outbox_trigger() {
    let root = temp_dir("noop-trigger");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    let conn = Connection::open(&path).unwrap();
    let plan = plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator).unwrap();
    for migration in plan.migrations.iter().take(13) {
        conn.execute_batch(migration.sql).unwrap();
    }
    conn.execute_batch(
        "DROP TRIGGER prodex_siem_outbox_claim_pair_update;
         CREATE TRIGGER prodex_siem_outbox_claim_pair_update
         BEFORE UPDATE OF claim_token, claim_expires_at_unix_ms ON prodex_siem_outbox
         BEGIN
             SELECT 1;
         END;",
    )
    .unwrap();
    drop(conn);

    let error = runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap_err();
    assert!(error.to_string().contains("trigger behavior"));

    std::fs::remove_dir_all(root).unwrap();
}

fn postgres_siem_outbox_fixture(client: &mut postgres::Client, schema: &str, event_id_type: &str) {
    client
        .batch_execute(&format!(
            "CREATE SCHEMA {schema};
             SET search_path TO {schema};
             CREATE TABLE prodex_tenants (
                 tenant_id UUID PRIMARY KEY
             );
             CREATE TABLE prodex_audit_log (
                 tenant_id UUID NOT NULL,
                 audit_event_id UUID NOT NULL,
                 PRIMARY KEY (tenant_id, audit_event_id)
             );
             CREATE TABLE prodex_siem_outbox (
                 tenant_id UUID NOT NULL REFERENCES prodex_tenants(tenant_id),
                 event_id {event_id_type} NOT NULL,
                 audit_event_id UUID NOT NULL,
                 event_envelope JSONB NOT NULL,
                 attempt_count INTEGER NOT NULL DEFAULT 0,
                 next_attempt_at_unix_ms BIGINT NOT NULL,
                 created_at_unix_ms BIGINT NOT NULL,
                 delivered_at_unix_ms BIGINT,
                 claim_token UUID,
                 claim_expires_at_unix_ms BIGINT,
                 PRIMARY KEY (tenant_id, event_id),
                 UNIQUE (tenant_id, audit_event_id),
                 FOREIGN KEY (tenant_id, audit_event_id)
                     REFERENCES prodex_audit_log(tenant_id, audit_event_id),
                 CHECK (attempt_count >= 0),
                 CHECK (
                     octet_length(event_envelope::text) <= 1048576
                     AND next_attempt_at_unix_ms >= 0
                     AND created_at_unix_ms >= 0
                     AND (
                         delivered_at_unix_ms IS NULL
                         OR delivered_at_unix_ms >= created_at_unix_ms
                     )
                 ),
                 CHECK (
                     (claim_token IS NULL AND claim_expires_at_unix_ms IS NULL)
                     OR (claim_token IS NOT NULL AND claim_expires_at_unix_ms IS NOT NULL)
                 )
             );
             CREATE INDEX prodex_siem_outbox_due_claim_idx
                 ON prodex_siem_outbox (
                     tenant_id, delivered_at_unix_ms, next_attempt_at_unix_ms,
                     claim_expires_at_unix_ms, event_id
                 );"
        ))
        .unwrap();
}

#[test]
#[ignore = "requires PRODEX_TEST_POSTGRES_URL"]
fn postgres_ledgerless_siem_shape_uses_catalog_semantics() {
    let url = std::env::var("PRODEX_TEST_POSTGRES_URL")
        .expect("PRODEX_TEST_POSTGRES_URL must point to the test PostgreSQL instance");
    let tls = prodex_storage_postgres_runtime::PostgresTlsConfig::explicit_disable();
    let mut client = prodex_storage_postgres_runtime::connect_blocking(&url, &tls).unwrap();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let accepted_schema = format!(
        "prodex_ledgerless_siem_accept_{}_{}",
        std::process::id(),
        stamp
    );
    postgres_siem_outbox_fixture(&mut client, &accepted_schema, "UUID");
    assert_eq!(infer_legacy_postgres_version(&mut client).unwrap(), 6);
    client
        .batch_execute(&format!(
            "DROP SCHEMA {accepted_schema} CASCADE; SET search_path TO public;"
        ))
        .unwrap();

    let cases = [
        (
            "future-column",
            "ALTER TABLE prodex_siem_outbox ADD COLUMN future_delivery_state TEXT;",
            "unknown columns",
            "UUID",
        ),
        (
            "future-index",
            "CREATE INDEX prodex_siem_outbox_future_idx ON prodex_siem_outbox (created_at_unix_ms);",
            "unknown index",
            "UUID",
        ),
        (
            "future-constraint",
            "ALTER TABLE prodex_siem_outbox ADD CONSTRAINT prodex_siem_outbox_future CHECK (event_id IS NOT NULL);",
            "unknown constraint",
            "UUID",
        ),
        (
            "wrong-index-order",
            "DROP INDEX prodex_siem_outbox_due_claim_idx;
             CREATE INDEX prodex_siem_outbox_due_claim_idx
                 ON prodex_siem_outbox (tenant_id, event_id, delivered_at_unix_ms, next_attempt_at_unix_ms, claim_expires_at_unix_ms);",
            "lease index",
            "UUID",
        ),
        (
            "wrong-column-type",
            "",
            "columns",
            "TEXT",
        ),
    ];
    for (index, (name, extra_sql, expected, event_id_type)) in cases.into_iter().enumerate() {
        let schema = format!(
            "prodex_ledgerless_siem_reject_{}_{}_{}",
            std::process::id(),
            stamp,
            index
        );
        postgres_siem_outbox_fixture(&mut client, &schema, event_id_type);
        if !extra_sql.is_empty() {
            client.batch_execute(extra_sql).unwrap();
        }
        let error = infer_legacy_postgres_version(&mut client)
            .unwrap_err()
            .to_string();
        client
            .batch_execute(&format!(
                "DROP SCHEMA {schema} CASCADE; SET search_path TO public;"
            ))
            .unwrap();
        assert!(error.contains(expected), "{name}: {error}");
    }
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

    drop(conn);
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
fn sqlite_enterprise_migrator_rejects_future_schema() {
    let root = temp_dir("future-version");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "INSERT INTO prodex_enterprise_schema_migrations
         (version, name, checksum, applied_at_epoch) VALUES (?1, 'future', 'future', 1)",
        [i64::from(REQUIRED_SQLITE_SCHEMA_VERSION.0) + 1],
    )
    .unwrap();
    drop(conn);

    let error = runtime_gateway_sqlite_migrate_enterprise_state(&path).unwrap_err();
    assert!(error.to_string().contains("newer than supported"));

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
