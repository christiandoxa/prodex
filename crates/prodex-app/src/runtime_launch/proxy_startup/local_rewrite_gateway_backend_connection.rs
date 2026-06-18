use std::path::Path;

use anyhow::{Context, Result, bail};
use postgres::{Client as PostgresClient, NoTls};
use redis::Commands;
use rusqlite::Connection;

use super::local_rewrite_gateway_sqlite_utils::runtime_gateway_sqlite_duplicate_column_error;

pub(super) fn runtime_gateway_sqlite_open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open gateway sqlite state {}", path.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
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
            call_id TEXT NOT NULL,
            key_name TEXT NOT NULL COLLATE NOCASE,
            model TEXT NOT NULL,
            minute_epoch INTEGER NOT NULL,
            input_tokens INTEGER NOT NULL,
            estimated_cost_microusd INTEGER,
            created_at_epoch INTEGER NOT NULL,
            UNIQUE(request_id, key_name, phase)
        );
        INSERT OR IGNORE INTO prodex_gateway_schema_migrations (version, applied_at_epoch)
        VALUES (1, strftime('%s', 'now'));
        "#,
    )?;
    for column in [
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN response_status INTEGER",
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN response_bytes INTEGER",
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN output_tokens INTEGER",
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN final_cost_microusd INTEGER",
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN final_cost_usd REAL",
        "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN reconciled_at_epoch INTEGER",
        "ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN tenant_id TEXT",
        "ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN team_id TEXT",
        "ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN project_id TEXT",
        "ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN user_id TEXT",
        "ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN budget_id TEXT",
        "ALTER TABLE prodex_gateway_scim_users ADD COLUMN tenant_id TEXT",
        "ALTER TABLE prodex_gateway_scim_users ADD COLUMN team_id TEXT",
        "ALTER TABLE prodex_gateway_scim_users ADD COLUMN project_id TEXT",
        "ALTER TABLE prodex_gateway_scim_users ADD COLUMN user_id TEXT",
        "ALTER TABLE prodex_gateway_scim_users ADD COLUMN budget_id TEXT",
    ] {
        match conn.execute(column, []) {
            Ok(_) => {}
            Err(err) if runtime_gateway_sqlite_duplicate_column_error(&err) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(conn)
}

pub(super) fn runtime_gateway_postgres_open(url: &str) -> Result<PostgresClient> {
    let mut client = PostgresClient::connect(url, NoTls)
        .context("failed to connect to gateway postgres state")?;
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
            UNIQUE(request_id, key_name, phase)
        );
        INSERT INTO prodex_gateway_schema_migrations (version, applied_at_epoch)
        VALUES (1, EXTRACT(EPOCH FROM now())::BIGINT)
        ON CONFLICT (version) DO NOTHING;
        ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN IF NOT EXISTS tenant_id TEXT;
        ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN IF NOT EXISTS team_id TEXT;
        ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN IF NOT EXISTS project_id TEXT;
        ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN IF NOT EXISTS user_id TEXT;
        ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN IF NOT EXISTS budget_id TEXT;
        ALTER TABLE prodex_gateway_scim_users ADD COLUMN IF NOT EXISTS tenant_id TEXT;
        ALTER TABLE prodex_gateway_scim_users ADD COLUMN IF NOT EXISTS team_id TEXT;
        ALTER TABLE prodex_gateway_scim_users ADD COLUMN IF NOT EXISTS project_id TEXT;
        ALTER TABLE prodex_gateway_scim_users ADD COLUMN IF NOT EXISTS user_id TEXT;
        ALTER TABLE prodex_gateway_scim_users ADD COLUMN IF NOT EXISTS budget_id TEXT;
        "#,
    )?;
    Ok(client)
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
    fn sqlite_open_bootstraps_schema() {
        let root = temp_dir("sqlite");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("state.sqlite");

        let conn = runtime_gateway_sqlite_open(&path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'prodex_gateway_virtual_keys'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        std::fs::remove_dir_all(root).unwrap();
    }
}
