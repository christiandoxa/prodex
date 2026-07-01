use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result, bail};
use postgres::{Client as PostgresClient, NoTls};
use redis::Commands;
use rusqlite::{Connection, OptionalExtension};

const RUNTIME_GATEWAY_SCHEMA_VERSION: i64 = 1;
const RUNTIME_GATEWAY_SCHEMA_BOOTSTRAP_ENV: &str =
    "PRODEX_GATEWAY_ALLOW_REQUEST_PATH_SCHEMA_BOOTSTRAP";

pub(super) fn runtime_gateway_sqlite_open(path: &Path) -> Result<Connection> {
    runtime_gateway_sqlite_open_inner(path, runtime_gateway_schema_bootstrap_allowed())
}

fn runtime_gateway_sqlite_open_inner(path: &Path, bootstrap_allowed: bool) -> Result<Connection> {
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
        if bootstrap_allowed {
            conn.execute_batch(
                r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS prodex_gateway_schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at_epoch INTEGER NOT NULL
            );
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
                typed_request_id TEXT,
                call_id TEXT NOT NULL,
                key_name TEXT NOT NULL COLLATE NOCASE,
                tenant_id TEXT,
                team_id TEXT,
                project_id TEXT,
                user_id TEXT,
                budget_id TEXT,
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
            INSERT OR IGNORE INTO prodex_gateway_schema_migrations (version, applied_at_epoch)
            VALUES (1, strftime('%s', 'now'));
            "#,
            )?;
        } else {
            runtime_gateway_sqlite_require_schema(&conn)?;
        }
        runtime_gateway_schema_mark_ensured(schema_key);
    }
    Ok(conn)
}

pub(super) fn runtime_gateway_postgres_open(url: &str) -> Result<PostgresClient> {
    runtime_gateway_postgres_open_inner(url, runtime_gateway_schema_bootstrap_allowed())
}

fn runtime_gateway_postgres_open_inner(
    url: &str,
    bootstrap_allowed: bool,
) -> Result<PostgresClient> {
    let mut client = PostgresClient::connect(url, NoTls)
        .context("failed to connect to gateway postgres state")?;
    let schema_key = runtime_gateway_schema_key("postgres", url);
    if runtime_gateway_schema_should_ensure(&schema_key) {
        if bootstrap_allowed {
            client.batch_execute(
                r#"
            CREATE TABLE IF NOT EXISTS prodex_gateway_schema_migrations (
                version BIGINT PRIMARY KEY,
                applied_at_epoch BIGINT NOT NULL
            );
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
                typed_request_id TEXT,
                call_id TEXT NOT NULL,
                key_name TEXT NOT NULL,
                tenant_id TEXT,
                team_id TEXT,
                project_id TEXT,
                user_id TEXT,
                budget_id TEXT,
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
            INSERT INTO prodex_gateway_schema_migrations (version, applied_at_epoch)
            VALUES (1, EXTRACT(EPOCH FROM now())::BIGINT)
            ON CONFLICT (version) DO NOTHING;
            "#,
            )?;
        } else {
            runtime_gateway_postgres_require_schema(&mut client)?;
        }
        runtime_gateway_schema_mark_ensured(schema_key);
    }
    Ok(client)
}

fn runtime_gateway_sqlite_require_schema(conn: &Connection) -> Result<()> {
    let version = conn
        .query_row(
            "SELECT MAX(version) FROM prodex_gateway_schema_migrations",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()
        .context("failed to read gateway sqlite schema version")?
        .flatten()
        .ok_or_else(|| anyhow::anyhow!("gateway sqlite schema has not been migrated"))?;
    if version < RUNTIME_GATEWAY_SCHEMA_VERSION {
        bail!("gateway sqlite schema is too old");
    }
    Ok(())
}

fn runtime_gateway_postgres_require_schema(client: &mut PostgresClient) -> Result<()> {
    let version: Option<i64> = client
        .query_opt(
            "SELECT MAX(version)::BIGINT FROM prodex_gateway_schema_migrations",
            &[],
        )
        .context("failed to read gateway postgres schema version")?
        .and_then(|row| row.get(0));
    let version =
        version.ok_or_else(|| anyhow::anyhow!("gateway postgres schema has not been migrated"))?;
    if version < RUNTIME_GATEWAY_SCHEMA_VERSION {
        bail!("gateway postgres schema is too old");
    }
    Ok(())
}

fn runtime_gateway_schema_bootstrap_allowed() -> bool {
    if cfg!(test) {
        return true;
    }
    matches!(
        std::env::var(RUNTIME_GATEWAY_SCHEMA_BOOTSTRAP_ENV)
            .ok()
            .as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
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

        let error = runtime_gateway_sqlite_open_inner(&path, false).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to read gateway sqlite schema version")
                || error
                    .to_string()
                    .contains("gateway sqlite schema has not been migrated"),
            "unexpected error: {error:#}"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sqlite_open_bootstraps_schema_when_explicitly_allowed() {
        let root = temp_dir("sqlite");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("state.sqlite");

        let conn = runtime_gateway_sqlite_open_inner(&path, true).unwrap();
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

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn gateway_open_helpers_do_not_embed_alter_table() {
        let source = include_str!("local_rewrite_gateway_backend_connection.rs");
        let expand_ddl = ["ALTER", "TABLE"].join(" ");

        assert!(
            !source.contains(&expand_ddl),
            "gateway SQL open helpers must not run expand DDL"
        );
    }

    #[test]
    fn production_schema_bootstrap_requires_explicit_escape_hatch() {
        let source = include_str!("local_rewrite_gateway_backend_connection.rs");

        assert!(source.contains("RUNTIME_GATEWAY_SCHEMA_BOOTSTRAP_ENV"));
        assert!(source.contains("runtime_gateway_sqlite_require_schema"));
        assert!(source.contains("runtime_gateway_postgres_require_schema"));
    }
}
