use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result, anyhow, bail};
use postgres::Client as PostgresClient;
#[cfg(test)]
use prodex_storage_sqlite::{SqliteRuntimeMode, plan_sqlite_migrations};
use redis::Commands;
use rusqlite::{Connection, OptionalExtension};

const RUNTIME_GATEWAY_SCHEMA_VERSION: i64 = 3;
const RUNTIME_GATEWAY_SQLITE_SCHEMA_MIGRATIONS_TABLE_SQL: &str = r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS prodex_gateway_schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at_epoch INTEGER NOT NULL
            );
            "#;
const RUNTIME_GATEWAY_POSTGRES_SCHEMA_MIGRATIONS_TABLE_SQL: &str = r#"
            CREATE TABLE IF NOT EXISTS prodex_gateway_schema_migrations (
                version BIGINT PRIMARY KEY,
                applied_at_epoch BIGINT NOT NULL
            );
            "#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeGatewayCompatibilityMigration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const RUNTIME_GATEWAY_SQLITE_COMPATIBILITY_MIGRATIONS: [RuntimeGatewayCompatibilityMigration; 3] = [
    RuntimeGatewayCompatibilityMigration {
        version: 1,
        name: "001_gateway_compatibility_schema",
        sql: r#"
            CREATE TABLE IF NOT EXISTS prodex_gateway_virtual_keys (
                name TEXT PRIMARY KEY COLLATE NOCASE,
                tenant_id TEXT,
                team_id TEXT,
                project_id TEXT,
                user_id TEXT,
                budget_id TEXT,
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
            CREATE TABLE IF NOT EXISTS prodex_gateway_scim_users (
                id TEXT PRIMARY KEY,
                user_name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                tenant_id TEXT,
                team_id TEXT,
                project_id TEXT,
                user_id TEXT,
                budget_id TEXT,
                external_id TEXT,
                display_name TEXT,
                active INTEGER NOT NULL DEFAULT 1,
                role TEXT,
                allowed_key_prefixes_json TEXT NOT NULL DEFAULT '[]',
                created_at_epoch INTEGER NOT NULL,
                updated_at_epoch INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS prodex_gateway_virtual_key_usage (
                key_name TEXT PRIMARY KEY COLLATE NOCASE,
                minute_epoch INTEGER NOT NULL DEFAULT 0,
                requests_this_minute INTEGER NOT NULL DEFAULT 0,
                tokens_this_minute INTEGER NOT NULL DEFAULT 0,
                requests_total INTEGER NOT NULL DEFAULT 0,
                spend_microusd INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS prodex_gateway_billing_ledger (
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
            CREATE INDEX IF NOT EXISTS prodex_gateway_virtual_keys_tenant_name_idx
                ON prodex_gateway_virtual_keys (tenant_id, name);
            CREATE INDEX IF NOT EXISTS prodex_gateway_scim_users_tenant_user_idx
                ON prodex_gateway_scim_users (tenant_id, user_name);
            CREATE UNIQUE INDEX IF NOT EXISTS prodex_gateway_billing_ledger_call_phase_idx
                ON prodex_gateway_billing_ledger (call_id, key_name, phase);
            "#,
    },
    RuntimeGatewayCompatibilityMigration {
        version: 2,
        name: "002_gateway_ledger_typed_request_and_scope_snapshot",
        sql: "",
    },
    RuntimeGatewayCompatibilityMigration {
        version: 3,
        name: "003_gateway_virtual_key_typed_id",
        sql: "",
    },
];
const RUNTIME_GATEWAY_POSTGRES_COMPATIBILITY_MIGRATIONS: [RuntimeGatewayCompatibilityMigration; 3] = [
    RuntimeGatewayCompatibilityMigration {
        version: 1,
        name: "001_gateway_compatibility_schema",
        sql: r#"
            CREATE TABLE IF NOT EXISTS prodex_gateway_virtual_keys (
                name TEXT PRIMARY KEY,
                tenant_id TEXT,
                team_id TEXT,
                project_id TEXT,
                user_id TEXT,
                budget_id TEXT,
                token_hash_base64 TEXT NOT NULL,
                allowed_models_json TEXT NOT NULL DEFAULT '[]',
                budget_microusd BIGINT,
                request_budget BIGINT,
                rpm_limit BIGINT,
                tpm_limit BIGINT,
                disabled BOOLEAN NOT NULL DEFAULT FALSE,
                created_at_epoch BIGINT NOT NULL,
                updated_at_epoch BIGINT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS prodex_gateway_scim_users (
                id TEXT PRIMARY KEY,
                user_name TEXT NOT NULL UNIQUE,
                tenant_id TEXT,
                team_id TEXT,
                project_id TEXT,
                user_id TEXT,
                budget_id TEXT,
                external_id TEXT,
                display_name TEXT,
                active BOOLEAN NOT NULL DEFAULT TRUE,
                role TEXT,
                allowed_key_prefixes_json TEXT NOT NULL DEFAULT '[]',
                created_at_epoch BIGINT NOT NULL,
                updated_at_epoch BIGINT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS prodex_gateway_virtual_key_usage (
                key_name TEXT PRIMARY KEY,
                minute_epoch BIGINT NOT NULL DEFAULT 0,
                requests_this_minute BIGINT NOT NULL DEFAULT 0,
                tokens_this_minute BIGINT NOT NULL DEFAULT 0,
                requests_total BIGINT NOT NULL DEFAULT 0,
                spend_microusd BIGINT NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS prodex_gateway_billing_ledger (
                id BIGSERIAL PRIMARY KEY,
                phase TEXT NOT NULL,
                request_id BIGINT NOT NULL,
                call_id TEXT NOT NULL,
                key_name TEXT NOT NULL,
                model TEXT NOT NULL,
                minute_epoch BIGINT NOT NULL,
                input_tokens BIGINT NOT NULL,
                estimated_cost_microusd BIGINT,
                created_at_epoch BIGINT NOT NULL,
                response_status BIGINT,
                response_bytes BIGINT,
                output_tokens BIGINT,
                final_cost_microusd BIGINT,
                final_cost_usd DOUBLE PRECISION,
                reconciled_at_epoch BIGINT,
                UNIQUE(call_id, key_name, phase)
            );
            CREATE INDEX IF NOT EXISTS prodex_gateway_virtual_keys_tenant_name_idx
                ON prodex_gateway_virtual_keys (tenant_id, name);
            CREATE INDEX IF NOT EXISTS prodex_gateway_scim_users_tenant_user_idx
                ON prodex_gateway_scim_users (tenant_id, user_name);
            CREATE UNIQUE INDEX IF NOT EXISTS prodex_gateway_billing_ledger_call_phase_idx
                ON prodex_gateway_billing_ledger (call_id, key_name, phase);
            "#,
    },
    RuntimeGatewayCompatibilityMigration {
        version: 2,
        name: "002_gateway_ledger_typed_request_and_scope_snapshot",
        sql: "",
    },
    RuntimeGatewayCompatibilityMigration {
        version: 3,
        name: "003_gateway_virtual_key_typed_id",
        sql: "",
    },
];

pub(super) fn runtime_gateway_sqlite_open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open gateway sqlite state {}", path.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    let schema_key = runtime_gateway_schema_key("sqlite", &path.display().to_string());
    if runtime_gateway_schema_should_ensure(&schema_key) {
        runtime_gateway_sqlite_require_schema(&conn)?;
        runtime_gateway_schema_mark_ensured(schema_key);
    }
    Ok(conn)
}

pub(super) fn runtime_gateway_postgres_open(
    url: &str,
    tls: &prodex_storage_postgres_runtime::PostgresTlsConfig,
) -> Result<PostgresClient> {
    let mut client = prodex_storage_postgres_runtime::connect_blocking(url, tls)
        .context("failed to connect to gateway postgres state")?;
    let schema_key = runtime_gateway_schema_key("postgres", url);
    if runtime_gateway_schema_should_ensure(&schema_key) {
        runtime_gateway_postgres_require_schema(&mut client)?;
        runtime_gateway_schema_mark_ensured(schema_key);
    }
    Ok(client)
}

fn runtime_gateway_sqlite_require_schema(conn: &Connection) -> Result<()> {
    let version = runtime_gateway_sqlite_observed_schema_version(conn)?
        .ok_or_else(|| anyhow!("gateway sqlite schema has not been migrated"))?;
    if version < RUNTIME_GATEWAY_SCHEMA_VERSION {
        bail!("gateway sqlite schema is too old");
    }
    runtime_gateway_sqlite_require_enterprise_schema(conn)?;
    Ok(())
}

fn runtime_gateway_postgres_require_schema(client: &mut PostgresClient) -> Result<()> {
    let version = runtime_gateway_postgres_observed_schema_version(client)?
        .ok_or_else(|| anyhow!("gateway postgres schema has not been migrated"))?;
    if version < RUNTIME_GATEWAY_SCHEMA_VERSION {
        bail!("gateway postgres schema is too old");
    }
    runtime_gateway_postgres_require_enterprise_schema(client)?;
    Ok(())
}

pub(crate) fn runtime_gateway_sqlite_migrate_compatibility_state(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open gateway sqlite state {}", path.display()))?;
    runtime_gateway_sqlite_apply_compatibility_migrations(&conn)?;
    Ok(())
}

pub(crate) fn runtime_gateway_postgres_migrate_compatibility_state(
    url: &str,
    tls: &prodex_storage_postgres_runtime::PostgresTlsConfig,
) -> Result<()> {
    let mut client = prodex_storage_postgres_runtime::connect_blocking(url, tls)
        .context("failed to connect to gateway postgres state")?;
    runtime_gateway_postgres_apply_compatibility_migrations(&mut client)?;
    Ok(())
}

#[cfg(test)]
pub(super) fn runtime_gateway_sqlite_create_current_schema_for_tests(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open gateway sqlite state {}", path.display()))?;
    let plan = plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator)?;
    for migration in &plan.migrations {
        conn.execute_batch(migration.sql).with_context(|| {
            format!(
                "failed to apply gateway sqlite enterprise migration {}",
                migration.name
            )
        })?;
    }
    drop(conn);
    runtime_gateway_sqlite_migrate_compatibility_state(path)
}

fn runtime_gateway_sqlite_apply_compatibility_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(RUNTIME_GATEWAY_SQLITE_SCHEMA_MIGRATIONS_TABLE_SQL)
        .context("failed to ensure gateway sqlite schema migrations table")?;
    let observed_version = runtime_gateway_sqlite_observed_schema_version(conn)?.unwrap_or(0);
    for migration in RUNTIME_GATEWAY_SQLITE_COMPATIBILITY_MIGRATIONS {
        if migration.version <= observed_version {
            continue;
        }
        let mut statement = conn
            .prepare("BEGIN IMMEDIATE")
            .context("failed to begin gateway sqlite compatibility migration transaction")?;
        statement.execute([])?;
        drop(statement);
        let apply_result = runtime_gateway_sqlite_apply_compatibility_migration(conn, migration)
            .and_then(|()| {
                conn.execute(
                    "INSERT OR IGNORE INTO prodex_gateway_schema_migrations (version, applied_at_epoch)
                     VALUES (?1, strftime('%s', 'now'))",
                    [migration.version],
                )
                .with_context(|| {
                    format!(
                        "failed to record gateway sqlite compatibility migration {}",
                        migration.name
                    )
                })?;
                conn.execute_batch("COMMIT").context(
                    "failed to commit gateway sqlite compatibility migration transaction",
                )?;
                Ok(())
            });
        if let Err(error) = apply_result {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(error);
        }
    }
    Ok(())
}

fn runtime_gateway_postgres_apply_compatibility_migrations(
    client: &mut PostgresClient,
) -> Result<()> {
    client
        .batch_execute(RUNTIME_GATEWAY_POSTGRES_SCHEMA_MIGRATIONS_TABLE_SQL)
        .context("failed to ensure gateway postgres schema migrations table")?;
    let observed_version = runtime_gateway_postgres_observed_schema_version(client)?.unwrap_or(0);
    for migration in RUNTIME_GATEWAY_POSTGRES_COMPATIBILITY_MIGRATIONS {
        if migration.version <= observed_version {
            continue;
        }
        let mut tx = client
            .transaction()
            .context("failed to begin gateway postgres compatibility migration transaction")?;
        runtime_gateway_postgres_apply_compatibility_migration(&mut tx, migration)?;
        tx.execute(
            "INSERT INTO prodex_gateway_schema_migrations (version, applied_at_epoch)
             VALUES ($1, EXTRACT(EPOCH FROM now())::BIGINT)
             ON CONFLICT (version) DO NOTHING",
            &[&migration.version],
        )
        .with_context(|| {
            format!(
                "failed to record gateway postgres compatibility migration {}",
                migration.name
            )
        })?;
        tx.commit()
            .context("failed to commit gateway postgres compatibility migration transaction")?;
    }
    Ok(())
}

fn runtime_gateway_sqlite_apply_compatibility_migration(
    conn: &Connection,
    migration: RuntimeGatewayCompatibilityMigration,
) -> Result<()> {
    match migration.version {
        1 => conn.execute_batch(migration.sql).with_context(|| {
            format!(
                "failed to apply gateway sqlite compatibility migration {}",
                migration.name
            )
        }),
        2 => runtime_gateway_sqlite_add_ledger_scope_columns(conn).with_context(|| {
            format!(
                "failed to apply gateway sqlite compatibility migration {}",
                migration.name
            )
        }),
        3 => runtime_gateway_sqlite_add_virtual_key_id_column(conn).with_context(|| {
            format!(
                "failed to apply gateway sqlite compatibility migration {}",
                migration.name
            )
        }),
        other => bail!("unsupported gateway sqlite compatibility migration {other}"),
    }
}

fn runtime_gateway_postgres_apply_compatibility_migration(
    tx: &mut postgres::Transaction<'_>,
    migration: RuntimeGatewayCompatibilityMigration,
) -> Result<()> {
    match migration.version {
        1 => tx.batch_execute(migration.sql).with_context(|| {
            format!(
                "failed to apply gateway postgres compatibility migration {}",
                migration.name
            )
        }),
        2 => runtime_gateway_postgres_add_ledger_scope_columns(tx).with_context(|| {
            format!(
                "failed to apply gateway postgres compatibility migration {}",
                migration.name
            )
        }),
        3 => runtime_gateway_postgres_add_virtual_key_id_column(tx).with_context(|| {
            format!(
                "failed to apply gateway postgres compatibility migration {}",
                migration.name
            )
        }),
        other => bail!("unsupported gateway postgres compatibility migration {other}"),
    }
}

fn runtime_gateway_sqlite_add_ledger_scope_columns(conn: &Connection) -> Result<()> {
    for (column_name, column_definition) in [
        ("typed_request_id", "TEXT"),
        ("tenant_id", "TEXT"),
        ("team_id", "TEXT"),
        ("project_id", "TEXT"),
        ("user_id", "TEXT"),
        ("budget_id", "TEXT"),
    ] {
        if runtime_gateway_sqlite_table_has_column(
            conn,
            "prodex_gateway_billing_ledger",
            column_name,
        )? {
            continue;
        }
        conn.execute_batch(&format!(
            "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN {column_name} {column_definition};"
        ))?;
    }
    Ok(())
}

fn runtime_gateway_postgres_add_ledger_scope_columns(
    tx: &mut postgres::Transaction<'_>,
) -> Result<()> {
    tx.batch_execute(
        r#"
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS typed_request_id TEXT;
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS tenant_id TEXT;
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS team_id TEXT;
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS project_id TEXT;
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS user_id TEXT;
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS budget_id TEXT;
        "#,
    )?;
    Ok(())
}

fn runtime_gateway_sqlite_add_virtual_key_id_column(conn: &Connection) -> Result<()> {
    let table_exists: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE type = 'table'
              AND name = 'prodex_gateway_virtual_keys'
        )",
        [],
        |row| row.get(0),
    )?;
    if !table_exists {
        return Ok(());
    }
    if !runtime_gateway_sqlite_table_has_column(
        conn,
        "prodex_gateway_virtual_keys",
        "virtual_key_id",
    )? {
        conn.execute_batch(
            "ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN virtual_key_id TEXT;",
        )?;
    }
    Ok(())
}

fn runtime_gateway_postgres_add_virtual_key_id_column(
    tx: &mut postgres::Transaction<'_>,
) -> Result<()> {
    tx.batch_execute(
        "ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN IF NOT EXISTS virtual_key_id TEXT;",
    )?;
    Ok(())
}

fn runtime_gateway_sqlite_observed_schema_version(conn: &Connection) -> Result<Option<i64>> {
    let has_migrations_table: bool = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM sqlite_master
                WHERE type = 'table'
                  AND name = 'prodex_gateway_schema_migrations'
            )",
            [],
            |row| row.get(0),
        )
        .context("failed to read gateway sqlite schema version")?;
    if !has_migrations_table {
        return Ok(None);
    }
    conn.query_row(
        "SELECT MAX(version) FROM prodex_gateway_schema_migrations",
        [],
        |row| row.get::<_, Option<i64>>(0),
    )
    .optional()
    .context("failed to read gateway sqlite schema version")
    .map(|value| value.flatten())
}

fn runtime_gateway_sqlite_require_enterprise_schema(conn: &Connection) -> Result<()> {
    for table_name in [
        "prodex_tenants",
        "prodex_budget_counters",
        "prodex_reservations",
        "prodex_usage_ledger",
    ] {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM sqlite_master
                WHERE type = 'table'
                  AND name = ?1
            )",
            [table_name],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("gateway sqlite enterprise accounting schema has not been migrated");
        }
    }
    Ok(())
}

fn runtime_gateway_postgres_observed_schema_version(
    client: &mut PostgresClient,
) -> Result<Option<i64>> {
    let has_migrations_table: bool = client
        .query_one(
            "SELECT EXISTS(
                SELECT 1
                FROM information_schema.tables
                WHERE table_schema = current_schema()
                  AND table_name = 'prodex_gateway_schema_migrations'
            )",
            &[],
        )
        .context("failed to read gateway postgres schema version")?
        .get(0);
    if !has_migrations_table {
        return Ok(None);
    }
    Ok(client
        .query_opt(
            "SELECT MAX(version)::BIGINT FROM prodex_gateway_schema_migrations",
            &[],
        )
        .context("failed to read gateway postgres schema version")?
        .and_then(|row| row.get(0)))
}

fn runtime_gateway_postgres_require_enterprise_schema(client: &mut PostgresClient) -> Result<()> {
    for table_name in [
        "prodex_tenants",
        "prodex_budget_counters",
        "prodex_reservations",
        "prodex_usage_ledger",
    ] {
        let exists: bool = client
            .query_one(
                "SELECT EXISTS(
                    SELECT 1
                    FROM information_schema.tables
                    WHERE table_schema = current_schema()
                      AND table_name = $1
                )",
                &[&table_name],
            )?
            .get(0);
        if !exists {
            bail!("gateway postgres enterprise accounting schema has not been migrated");
        }
    }
    Ok(())
}

fn runtime_gateway_sqlite_table_has_column(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool> {
    let count: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM pragma_table_info('{table_name}') WHERE name = ?1"),
        [column_name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn runtime_gateway_schema_key(kind: &str, identity: &str) -> String {
    format!("{kind}:{identity}")
}

fn runtime_gateway_schema_should_ensure(schema_key: &str) -> bool {
    let ensured = runtime_gateway_schema_ensured_keys()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    !ensured.contains(schema_key)
}

fn runtime_gateway_schema_mark_ensured(schema_key: String) {
    let mut ensured = runtime_gateway_schema_ensured_keys()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    ensured.insert(schema_key);
}

fn runtime_gateway_schema_ensured_keys() -> &'static Mutex<BTreeSet<String>> {
    static ENSURED: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
    ENSURED.get_or_init(|| Mutex::new(BTreeSet::new()))
}
pub(super) fn runtime_gateway_redis_connection(url: &str) -> Result<redis::Connection> {
    let client = redis::Client::open(url).context("failed to open gateway redis client")?;
    client
        .get_connection()
        .context("failed to connect to gateway redis state")
}

pub(super) fn runtime_gateway_redis_with_lock<F, G, T>(
    url: &str,
    lock_key: &str,
    token_generator: G,
    operation: F,
) -> Result<T>
where
    F: FnOnce(&mut redis::Connection) -> Result<T>,
    G: FnOnce() -> Result<String>,
{
    let token = token_generator()?;
    let mut conn = runtime_gateway_redis_connection(url)?;
    let mut acquired = false;
    for _ in 0..50 {
        let result: Option<String> = redis::cmd("SET")
            .arg(lock_key)
            .arg(&token)
            .arg("NX")
            .arg("PX")
            .arg(5_000)
            .query(&mut conn)?;
        if result.is_some() {
            acquired = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if !acquired {
        bail!("timed out waiting for redis lock {lock_key}");
    }

    let result = operation(&mut conn);
    let release_result: redis::RedisResult<()> = conn.del(lock_key);
    if result.is_ok() {
        release_result.context("failed to release gateway redis lock")?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
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
                .find("fn runtime_gateway_sqlite_require_schema")
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
        let sqlite_bootstrap_const =
            ["const RUNTIME_GATEWAY_SQLITE_BOOTSTRAP_SCHEMA", "_SQL"].concat();
        let postgres_bootstrap_const =
            ["const RUNTIME_GATEWAY_POSTGRES_BOOTSTRAP_SCHEMA", "_SQL"].concat();
        let tests_start = source.rfind("#[cfg(test)]\nmod tests {").unwrap();
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
}
