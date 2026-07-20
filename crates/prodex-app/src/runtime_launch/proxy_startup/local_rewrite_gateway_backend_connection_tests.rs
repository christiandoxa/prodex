use super::{
    RUNTIME_GATEWAY_SCHEMA_VERSION, RUNTIME_GATEWAY_SQLITE_COMPATIBILITY_MIGRATIONS,
    runtime_gateway_schema_key, runtime_gateway_schema_mark_ensured,
    runtime_gateway_schema_should_ensure, runtime_gateway_sqlite_create_current_schema_for_tests,
    runtime_gateway_sqlite_migrate_compatibility_state, runtime_gateway_sqlite_open,
};
use rusqlite::Connection;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("prodex-gateway-backend-connection-{name}-{stamp}"))
}

fn create_legacy_sqlite_compatibility_schema(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        r#"
            CREATE TABLE prodex_gateway_schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at_epoch INTEGER NOT NULL
            );
            INSERT INTO prodex_gateway_schema_migrations (version, applied_at_epoch)
            VALUES (1, 1);
            CREATE TABLE prodex_gateway_virtual_keys (
                name TEXT PRIMARY KEY COLLATE NOCASE,
                token_hash_base64 TEXT NOT NULL,
                allowed_models_json TEXT NOT NULL DEFAULT '[]',
                budget_microusd INTEGER,
                request_budget INTEGER,
                rpm_limit INTEGER,
                tpm_limit INTEGER,
                disabled INTEGER NOT NULL DEFAULT 0,
                created_at_epoch INTEGER NOT NULL,
                updated_at_epoch INTEGER NOT NULL
            );
            CREATE TABLE prodex_gateway_scim_users (
                id TEXT PRIMARY KEY,
                user_name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                external_id TEXT,
                display_name TEXT,
                active INTEGER NOT NULL DEFAULT 1,
                role TEXT,
                allowed_key_prefixes_json TEXT NOT NULL DEFAULT '[]',
                created_at_epoch INTEGER NOT NULL,
                updated_at_epoch INTEGER NOT NULL
            );
            CREATE TABLE prodex_gateway_virtual_key_usage (
                key_name TEXT PRIMARY KEY COLLATE NOCASE,
                minute_epoch INTEGER NOT NULL DEFAULT 0,
                requests_this_minute INTEGER NOT NULL DEFAULT 0,
                tokens_this_minute INTEGER NOT NULL DEFAULT 0,
                requests_total INTEGER NOT NULL DEFAULT 0,
                spend_microusd INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE prodex_gateway_billing_ledger (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                phase TEXT NOT NULL,
                request_id INTEGER NOT NULL,
                call_id TEXT NOT NULL,
                key_name TEXT NOT NULL COLLATE NOCASE,
                model TEXT NOT NULL,
                minute_epoch INTEGER NOT NULL,
                input_tokens INTEGER NOT NULL,
                estimated_cost_microusd INTEGER,
                created_at_epoch INTEGER NOT NULL,
                response_status INTEGER,
                response_bytes INTEGER,
                output_tokens INTEGER,
                final_cost_microusd INTEGER,
                final_cost_usd REAL,
                reconciled_at_epoch INTEGER,
                UNIQUE(call_id, key_name, phase)
            );
            "#,
    )
    .unwrap();
}

#[test]
fn schema_guard_marks_successful_backend_key_once() {
    let key = runtime_gateway_schema_key(
        "sqlite-test",
        &format!(
            "state-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ),
    );

    assert!(runtime_gateway_schema_should_ensure(&key));
    assert!(runtime_gateway_schema_should_ensure(&key));

    runtime_gateway_schema_mark_ensured(key.clone());

    assert!(!runtime_gateway_schema_should_ensure(&key));
}

#[test]
fn sqlite_open_rejects_unmigrated_store_by_default() {
    let root = temp_dir("sqlite-unmigrated");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");

    let error = runtime_gateway_sqlite_open(&path).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("gateway sqlite schema has not been migrated"),
        "unexpected error: {error:#}"
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_open_accepts_versioned_compatibility_schema() {
    let root = temp_dir("sqlite");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");

    runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
    let conn = runtime_gateway_sqlite_open(&path).unwrap();
    let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'prodex_gateway_virtual_keys'",
                [],
                |row| row.get(0),
            )
            .unwrap();
    assert_eq!(count, 1);
    for index_name in [
        "prodex_gateway_virtual_keys_tenant_name_idx",
        "prodex_gateway_scim_users_tenant_user_idx",
    ] {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [index_name],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "{index_name} should exist");
    }
    for column_name in [
        "typed_request_id",
        "tenant_id",
        "team_id",
        "project_id",
        "user_id",
        "budget_id",
    ] {
        let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('prodex_gateway_billing_ledger') WHERE name = ?1",
                    [column_name],
                    |row| row.get(0),
                )
                .unwrap();
        assert_eq!(count, 1, "{column_name} should exist");
    }
    let virtual_key_id_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('prodex_gateway_virtual_keys') WHERE name = 'virtual_key_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
    assert_eq!(virtual_key_id_count, 1);
    for column_name in ["group_ids_json", "department_id"] {
        let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('prodex_gateway_scim_users') WHERE name = ?1",
                    [column_name],
                    |row| row.get(0),
                )
                .unwrap();
        assert_eq!(count, 1, "{column_name} should exist");
    }
    let enterprise_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'prodex_tenants'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(enterprise_count, 1);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_open_rejects_compatibility_only_schema_without_enterprise_tables() {
    let root = temp_dir("sqlite-compatibility-only");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");

    runtime_gateway_sqlite_migrate_compatibility_state(&path).unwrap();
    let error = runtime_gateway_sqlite_open(&path).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("gateway sqlite enterprise accounting schema has not been migrated"),
        "unexpected error: {error:#}"
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_open_rejects_old_compatibility_schema_version() {
    let root = temp_dir("sqlite-old-version");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    create_legacy_sqlite_compatibility_schema(&path);

    let error = runtime_gateway_sqlite_open(&path).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("gateway sqlite schema is too old")
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_compatibility_migrations_are_versioned_and_idempotent() {
    let root = temp_dir("sqlite-versioned");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");

    runtime_gateway_sqlite_migrate_compatibility_state(&path).unwrap();
    runtime_gateway_sqlite_migrate_compatibility_state(&path).unwrap();

    let conn = Connection::open(&path).unwrap();
    let migration_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prodex_gateway_schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let max_version: i64 = conn
        .query_row(
            "SELECT MAX(version) FROM prodex_gateway_schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(
        migration_count,
        RUNTIME_GATEWAY_SQLITE_COMPATIBILITY_MIGRATIONS.len() as i64
    );
    assert_eq!(max_version, RUNTIME_GATEWAY_SCHEMA_VERSION);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn gateway_request_path_open_helpers_keep_expand_ddl_out_of_open_paths() {
    let source = include_str!("local_rewrite_gateway_backend_connection.rs");
    let sqlite_open_start = source
        .find("pub(super) fn runtime_gateway_sqlite_open")
        .unwrap();
    let postgres_open_start = source
        .find("pub(super) fn runtime_gateway_postgres_open")
        .unwrap();
    let sqlite_open_body = &source[sqlite_open_start..postgres_open_start];
    let postgres_open_body = &source[postgres_open_start
        ..source
            .find("pub(crate) fn runtime_gateway_sqlite_migrate_compatibility_state")
            .unwrap()];
    let expand_ddl = "ALTER TABLE";

    assert!(
        !sqlite_open_body.contains(expand_ddl),
        "gateway SQL open helpers must not run expand DDL"
    );
    assert!(
        !postgres_open_body.contains(expand_ddl),
        "gateway SQL open helpers must not run expand DDL"
    );
    assert!(!sqlite_open_body.contains("migrate_compatibility_state"));
    assert!(!postgres_open_body.contains("migrate_compatibility_state"));
}

#[test]
fn production_gateway_open_paths_no_longer_advertise_bootstrap_escape_hatch() {
    let source = include_str!("local_rewrite_gateway_backend_connection.rs");
    let sqlite_bootstrap_const = ["const RUNTIME_GATEWAY_SQLITE_BOOTSTRAP_SCHEMA", "_SQL"].concat();
    let postgres_bootstrap_const =
        ["const RUNTIME_GATEWAY_POSTGRES_BOOTSTRAP_SCHEMA", "_SQL"].concat();
    let tests_start = source
        .rfind("\n#[cfg(test)]")
        .expect("external test module marker should exist");
    let production = &source[..tests_start];

    assert!(!production.contains("PRODEX_GATEWAY_ALLOW_REQUEST_PATH_SCHEMA_BOOTSTRAP"));
    assert!(production.contains("runtime_gateway_sqlite_require_schema"));
    assert!(production.contains("runtime_gateway_postgres_require_schema"));
    assert!(production.contains("runtime_gateway_sqlite_migrate_compatibility_state"));
    assert!(production.contains("runtime_gateway_postgres_migrate_compatibility_state"));
    assert!(production.contains("RUNTIME_GATEWAY_SQLITE_COMPATIBILITY_MIGRATIONS"));
    assert!(production.contains("RUNTIME_GATEWAY_POSTGRES_COMPATIBILITY_MIGRATIONS"));
    assert!(!production.contains(&sqlite_bootstrap_const));
    assert!(!production.contains(&postgres_bootstrap_const));
}
