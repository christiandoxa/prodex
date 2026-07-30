use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use anyhow::{Context, Result};
use redis::Commands;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, types::Type};

#[path = "local_rewrite_gateway_usage_backend_redis_legacy.rs"]
mod redis_legacy_migration;

use super::local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_open, runtime_gateway_redis_connection, runtime_gateway_sqlite_open,
};
use super::local_rewrite_gateway_ledger_types::runtime_gateway_billing_ledger_entry_from_delta;
use super::local_rewrite_gateway_redis_ledger::{
    runtime_gateway_redis_ledger_call_index_key, runtime_gateway_redis_ledger_entry_id,
    runtime_gateway_redis_ledger_entry_key, runtime_gateway_redis_ledger_id_index_key,
    runtime_gateway_redis_ledger_index_key,
    runtime_gateway_redis_migrate_legacy_ledger_from_connection,
};
use super::local_rewrite_gateway_sqlite_utils::{
    runtime_gateway_sqlite_optional_u64_to_i64, runtime_gateway_sqlite_u64_to_i64,
};
use redis_legacy_migration::runtime_gateway_redis_migrate_legacy_usage_from_connection;
#[cfg(test)]
use redis_legacy_migration::{
    RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_BEGIN_SCRIPT,
    RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_FINALIZE_SCRIPT,
    RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATE_SCRIPT,
    RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATION_MARKER_VALUE,
    runtime_gateway_redis_legacy_usage_fingerprint, runtime_gateway_redis_usage_migrated_keys_key,
    runtime_gateway_redis_usage_migration_in_progress_key,
    runtime_gateway_redis_usage_migration_marker_key,
};

#[derive(Clone)]
pub(super) struct RuntimeGatewayVirtualKeyUsageDelta {
    pub(super) request_id: u64,
    pub(super) typed_request_id: String,
    pub(super) call_id: String,
    pub(super) key_name: String,
    pub(super) tenant_id: Option<String>,
    pub(super) team_id: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) user_id: Option<String>,
    pub(super) budget_id: Option<String>,
    pub(super) model: String,
    pub(super) minute_epoch: u64,
    pub(super) input_tokens: u64,
    pub(super) reserved_tokens: u64,
    pub(super) estimated_cost_microusd: Option<u64>,
    pub(super) created_at_epoch: u64,
}

impl fmt::Debug for RuntimeGatewayVirtualKeyUsageDelta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayVirtualKeyUsageDelta")
            .field("request_id", &"<redacted>")
            .field("typed_request_id", &"<redacted>")
            .field("call_id", &"<redacted>")
            .field("key_name", &"<redacted>")
            .field("tenant_id", &redacted_option(&self.tenant_id))
            .field("team_id", &redacted_option(&self.team_id))
            .field("project_id", &redacted_option(&self.project_id))
            .field("user_id", &redacted_option(&self.user_id))
            .field("budget_id", &redacted_option(&self.budget_id))
            .field("model", &"<redacted>")
            .field("minute_epoch", &"<redacted>")
            .field("input_tokens", &"<redacted>")
            .field("reserved_tokens", &"<redacted>")
            .field(
                "estimated_cost_microusd",
                &redacted_option(&self.estimated_cost_microusd),
            )
            .field("created_at_epoch", &"<redacted>")
            .finish()
    }
}

fn redacted_option<T>(value: &Option<T>) -> Option<&'static str> {
    value.as_ref().map(|_| "<redacted>")
}

pub(super) fn runtime_gateway_sqlite_usage_load(
    path: &Path,
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let conn = runtime_gateway_sqlite_open(path)?;
    runtime_gateway_sqlite_usage_load_from_conn(&conn)
}

pub(super) fn runtime_gateway_sqlite_usage_load_from_conn(
    conn: &Connection,
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT key_name, minute_epoch, requests_this_minute, tokens_this_minute,
               requests_total, spend_microusd
        FROM prodex_gateway_virtual_key_usage
        ORDER BY key_name COLLATE NOCASE
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            runtime_gateway_sqlite_usage_from_row(row)?,
        ))
    })?;
    let mut usage = BTreeMap::new();
    for row in rows {
        let (key_name, key_usage) = row?;
        usage.insert(key_name, key_usage);
    }
    Ok(usage)
}

pub(super) fn runtime_gateway_postgres_usage_load(
    url: &str,
    tls: &prodex_storage_postgres_runtime::PostgresTlsConfig,
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let mut client = runtime_gateway_postgres_open(url, tls)?;
    let rows = client.query(
        r#"
        SELECT key_name, minute_epoch, requests_this_minute, tokens_this_minute,
               requests_total, spend_microusd
        FROM prodex_gateway_virtual_key_usage
        ORDER BY lower(key_name), key_name
        "#,
        &[],
    )?;
    let mut usage = BTreeMap::new();
    for row in rows {
        usage.insert(row.get(0), runtime_gateway_postgres_usage_from_row(&row)?);
    }
    Ok(usage)
}

pub(super) fn runtime_gateway_redis_usage_load(
    url: &str,
    redis_key: &str,
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let mut conn = runtime_gateway_redis_connection(url)?;
    runtime_gateway_redis_migrate_legacy_usage_from_connection(&mut conn, redis_key)?;
    let index_key = runtime_gateway_redis_usage_index_key(redis_key);
    let names: Vec<String> = conn.smembers(&index_key)?;
    let mut usage = BTreeMap::new();

    for name in names {
        let hash_key = runtime_gateway_redis_usage_hash_key(redis_key, &name);
        let fields: BTreeMap<String, String> = conn.hgetall(&hash_key)?;
        if fields.is_empty() {
            anyhow::bail!("gateway redis usage index references a missing counter");
        }
        usage.insert(name, runtime_gateway_redis_usage_from_hash(&fields)?);
    }
    Ok(usage)
}

fn runtime_gateway_redis_usage_index_key(redis_key: &str) -> String {
    format!("{redis_key}:keys")
}

fn runtime_gateway_redis_usage_hash_key(redis_key: &str, key_name: &str) -> String {
    format!("{redis_key}:key:{key_name}")
}

fn runtime_gateway_redis_usage_from_hash(
    fields: &BTreeMap<String, String>,
) -> Result<runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage> {
    Ok(runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
        minute_epoch: runtime_gateway_redis_hash_u64(fields, "minute_epoch")?,
        requests_this_minute: runtime_gateway_redis_hash_u64(fields, "requests_this_minute")?,
        tokens_this_minute: runtime_gateway_redis_hash_u64(fields, "tokens_this_minute")?,
        requests_total: runtime_gateway_redis_hash_u64(fields, "requests_total")?,
        spend_microusd: runtime_gateway_redis_hash_u64(fields, "spend_microusd")?,
    })
}

fn runtime_gateway_redis_hash_u64(fields: &BTreeMap<String, String>, name: &str) -> Result<u64> {
    let Some(value) = fields.get(name) else {
        return Ok(0);
    };
    if value.is_empty() {
        return Ok(0);
    }
    if value.chars().any(char::is_whitespace) {
        anyhow::bail!("gateway redis usage field {name} must not contain whitespace");
    }
    value
        .parse::<u64>()
        .with_context(|| format!("gateway redis usage field {name} must be an unsigned integer"))
}

pub(super) fn runtime_gateway_sqlite_usage_apply_deltas(
    path: &Path,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()> {
    let mut conn = runtime_gateway_sqlite_open(path)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for delta in deltas {
        let ledger = runtime_gateway_billing_ledger_entry_from_delta(delta);
        let inserted = tx.execute(
            r#"
                INSERT OR IGNORE INTO prodex_gateway_billing_ledger (
                    phase, request_id, typed_request_id, call_id, key_name,
                    tenant_id, team_id, project_id, user_id, budget_id, model, minute_epoch,
                    input_tokens, reserved_tokens, estimated_cost_microusd, created_at_epoch
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                "#,
            params![
                ledger.phase,
                runtime_gateway_sqlite_u64_to_i64(ledger.request),
                ledger.request_id,
                ledger.call_id,
                ledger.key_name,
                ledger.tenant_id,
                ledger.team_id,
                ledger.project_id,
                ledger.user_id,
                ledger.budget_id,
                ledger.model,
                runtime_gateway_sqlite_u64_to_i64(ledger.minute_epoch),
                runtime_gateway_sqlite_u64_to_i64(ledger.input_tokens),
                runtime_gateway_sqlite_optional_u64_to_i64(ledger.reserved_tokens),
                runtime_gateway_sqlite_optional_u64_to_i64(ledger.estimated_cost_microusd),
                runtime_gateway_sqlite_u64_to_i64(ledger.created_at_epoch),
            ],
        )?;
        if inserted == 0 {
            continue;
        }
        let mut usage = tx
            .query_row(
                r#"
                SELECT key_name, minute_epoch, requests_this_minute, tokens_this_minute,
                       requests_total, spend_microusd
                FROM prodex_gateway_virtual_key_usage
                WHERE key_name = ?1
                "#,
                params![delta.key_name],
                runtime_gateway_sqlite_usage_from_row,
            )
            .optional()?
            .unwrap_or_default();
        prodex_gateway_core::apply_gateway_virtual_key_usage_update(
            &mut usage,
            prodex_gateway_core::GatewayVirtualKeyUsageUpdate {
                minute_epoch: delta.minute_epoch,
                reserved_tokens: delta.reserved_tokens,
                estimated_cost_microusd: delta.estimated_cost_microusd,
            },
        );
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_virtual_key_usage (
                key_name, minute_epoch, requests_this_minute, tokens_this_minute,
                requests_total, spend_microusd
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(key_name) DO UPDATE SET
                minute_epoch = excluded.minute_epoch,
                requests_this_minute = excluded.requests_this_minute,
                tokens_this_minute = excluded.tokens_this_minute,
                requests_total = excluded.requests_total,
                spend_microusd = excluded.spend_microusd
            "#,
            params![
                delta.key_name,
                runtime_gateway_sqlite_u64_to_i64(usage.minute_epoch),
                runtime_gateway_sqlite_u64_to_i64(usage.requests_this_minute),
                runtime_gateway_sqlite_u64_to_i64(usage.tokens_this_minute),
                runtime_gateway_sqlite_u64_to_i64(usage.requests_total),
                runtime_gateway_sqlite_u64_to_i64(usage.spend_microusd),
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub(super) fn runtime_gateway_postgres_usage_apply_deltas(
    url: &str,
    tls: &prodex_storage_postgres_runtime::PostgresTlsConfig,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()> {
    let mut client = runtime_gateway_postgres_open(url, tls)?;
    let mut tx = client.transaction()?;
    for delta in deltas {
        let ledger = runtime_gateway_billing_ledger_entry_from_delta(delta);
        let inserted = tx.execute(
            RUNTIME_GATEWAY_POSTGRES_LEDGER_INSERT_SQL,
            &[
                &ledger.phase,
                &runtime_gateway_sqlite_u64_to_i64(ledger.request),
                &ledger.request_id,
                &ledger.call_id,
                &ledger.key_name,
                &ledger.tenant_id,
                &ledger.team_id,
                &ledger.project_id,
                &ledger.user_id,
                &ledger.budget_id,
                &ledger.model,
                &runtime_gateway_sqlite_u64_to_i64(ledger.minute_epoch),
                &runtime_gateway_sqlite_u64_to_i64(ledger.input_tokens),
                &runtime_gateway_sqlite_optional_u64_to_i64(ledger.reserved_tokens),
                &runtime_gateway_sqlite_optional_u64_to_i64(ledger.estimated_cost_microusd),
                &runtime_gateway_sqlite_u64_to_i64(ledger.created_at_epoch),
            ],
        )?;
        if inserted == 0 {
            continue;
        }
        tx.execute(
            RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL,
            &[
                &delta.key_name,
                &runtime_gateway_sqlite_u64_to_i64(delta.minute_epoch),
                &runtime_gateway_sqlite_u64_to_i64(delta.reserved_tokens),
                &runtime_gateway_sqlite_u64_to_i64(delta.estimated_cost_microusd.unwrap_or(0)),
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

const RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL: &str = r#"
            INSERT INTO prodex_gateway_virtual_key_usage (
                key_name, minute_epoch, requests_this_minute, tokens_this_minute,
                requests_total, spend_microusd
            )
            VALUES ($1, $2, 1, $3, 1, $4)
            ON CONFLICT(key_name) DO UPDATE SET
                minute_epoch = EXCLUDED.minute_epoch,
                requests_this_minute = CASE
                    WHEN prodex_gateway_virtual_key_usage.minute_epoch = EXCLUDED.minute_epoch
                    THEN prodex_gateway_virtual_key_usage.requests_this_minute + 1
                    ELSE 1
                END,
                tokens_this_minute = CASE
                    WHEN prodex_gateway_virtual_key_usage.minute_epoch = EXCLUDED.minute_epoch
                    THEN prodex_gateway_virtual_key_usage.tokens_this_minute + EXCLUDED.tokens_this_minute
                    ELSE EXCLUDED.tokens_this_minute
                END,
                requests_total = prodex_gateway_virtual_key_usage.requests_total + 1,
                spend_microusd = prodex_gateway_virtual_key_usage.spend_microusd + EXCLUDED.spend_microusd
            "#;

const RUNTIME_GATEWAY_POSTGRES_LEDGER_INSERT_SQL: &str = r#"
            INSERT INTO prodex_gateway_billing_ledger (
                phase, request_id, typed_request_id, call_id, key_name,
                tenant_id, team_id, project_id, user_id, budget_id, model, minute_epoch,
                input_tokens, reserved_tokens, estimated_cost_microusd, created_at_epoch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT(call_id, key_name, phase) DO NOTHING
            "#;

const RUNTIME_GATEWAY_REDIS_USAGE_APPLY_SCRIPT: &str = r#"
        local function valid_type(key, expected)
            local actual = redis.call('TYPE', key).ok
            return actual == 'none' or actual == expected
        end
        if not valid_type(KEYS[1], 'set')
            or not valid_type(KEYS[2], 'hash')
            or not valid_type(KEYS[3], 'list')
            or not valid_type(KEYS[4], 'set')
            or not valid_type(KEYS[5], 'string')
            or not valid_type(KEYS[6], 'set') then
            return redis.error_reply('WRONGTYPE gateway accounting key')
        end
        local usage_member = redis.call('SISMEMBER', KEYS[1], ARGV[1])
        local usage_exists = redis.call('EXISTS', KEYS[2])
        if usage_member ~= usage_exists then
            return redis.error_reply('gateway accounting usage index is inconsistent')
        end
        local function ledger_identity(payload)
            local decoded, entry = pcall(cjson.decode, payload)
            if not decoded or type(entry) ~= 'table'
                or type(entry.call_id) ~= 'string'
                or type(entry.key_name) ~= 'string'
                or type(entry.phase) ~= 'string' then
                return nil
            end
            return string.len(entry.call_id) .. ':' .. entry.call_id
                .. string.len(entry.key_name) .. ':' .. entry.key_name
                .. string.len(entry.phase) .. ':' .. entry.phase
        end
        local entry_exists = redis.call('EXISTS', KEYS[5])
        local call_member = redis.call('SISMEMBER', KEYS[4], ARGV[5])
        local global_member = redis.call('SISMEMBER', KEYS[6], ARGV[5])
        if entry_exists == 1 then
            local existing_payload = redis.call('GET', KEYS[5])
            if usage_exists ~= 1 or call_member ~= 1 or global_member ~= 1
                or ledger_identity(existing_payload) ~= ARGV[5] then
                return redis.error_reply('gateway accounting ledger index is inconsistent')
            end
            return 0
        end
        if call_member ~= 0 or global_member ~= 0 then
            return redis.error_reply('gateway accounting ledger index is inconsistent')
        end

        local function normalize(value)
            if value == false or value == nil or value == '' then
                return '0'
            end
            if string.match(value, '^%d+$') == nil then
                return nil
            end
            value = string.gsub(value, '^0+', '')
            if value == '' then
                return '0'
            end
            return value
        end
        local function add_decimal(left, right)
            left = normalize(left)
            right = normalize(right)
            if left == nil or right == nil then
                return nil
            end
            local result = ''
            local carry = 0
            local left_index = string.len(left)
            local right_index = string.len(right)
            while left_index > 0 or right_index > 0 or carry > 0 do
                local left_digit = 0
                local right_digit = 0
                if left_index > 0 then
                    left_digit = string.byte(left, left_index) - 48
                    left_index = left_index - 1
                end
                if right_index > 0 then
                    right_digit = string.byte(right, right_index) - 48
                    right_index = right_index - 1
                end
                local total = left_digit + right_digit + carry
                result = string.char(48 + (total % 10)) .. result
                carry = math.floor(total / 10)
            end
            if string.len(result) > 19
                or (string.len(result) == 19 and result > '9223372036854775807') then
                return nil
            end
            return result
        end

        local current_minute = '0'
        local current_requests = '0'
        local current_tokens = '0'
        local current_total = '0'
        local current_spend = '0'
        if redis.call('EXISTS', KEYS[2]) == 1 then
            current_minute = redis.call('HGET', KEYS[2], 'minute_epoch')
            current_requests = redis.call('HGET', KEYS[2], 'requests_this_minute')
            current_tokens = redis.call('HGET', KEYS[2], 'tokens_this_minute')
            current_total = redis.call('HGET', KEYS[2], 'requests_total')
            current_spend = redis.call('HGET', KEYS[2], 'spend_microusd')
        end
        current_minute = normalize(current_minute)
        current_requests = normalize(current_requests)
        current_tokens = normalize(current_tokens)
        current_total = normalize(current_total)
        current_spend = normalize(current_spend)
        local minute = normalize(ARGV[2])
        local reserved_tokens = normalize(ARGV[3])
        local spend = normalize(ARGV[4])
        if current_minute == nil or current_requests == nil or current_tokens == nil
            or current_total == nil or current_spend == nil or minute == nil
            or reserved_tokens == nil or spend == nil then
            return redis.error_reply('gateway accounting counter is malformed')
        end

        local next_requests = '1'
        local next_tokens = reserved_tokens
        if current_minute == minute then
            next_requests = add_decimal(current_requests, '1')
            next_tokens = add_decimal(current_tokens, reserved_tokens)
        end
        local next_total = add_decimal(current_total, '1')
        local next_spend = add_decimal(current_spend, spend)
        if next_requests == nil or next_tokens == nil or next_total == nil or next_spend == nil then
            return redis.error_reply('gateway accounting counter overflow')
        end

        redis.call('SET', KEYS[5], ARGV[6])
        redis.call('RPUSH', KEYS[3], ARGV[5])
        redis.call('SADD', KEYS[4], ARGV[5])
        redis.call('SADD', KEYS[6], ARGV[5])
        redis.call('SADD', KEYS[1], ARGV[1])
        redis.call(
            'HSET', KEYS[2],
            'minute_epoch', minute,
            'requests_this_minute', next_requests,
            'tokens_this_minute', next_tokens,
            'requests_total', next_total,
            'spend_microusd', next_spend
        )
        return 1
        "#;

pub(super) fn runtime_gateway_redis_usage_apply_deltas(
    url: &str,
    usage_key: &str,
    ledger_key: &str,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()> {
    let mut conn = runtime_gateway_redis_connection(url)?;
    runtime_gateway_redis_migrate_legacy_ledger_from_connection(&mut conn, ledger_key)?;
    runtime_gateway_redis_migrate_legacy_usage_from_connection(&mut conn, usage_key)?;
    let usage_index_key = runtime_gateway_redis_usage_index_key(usage_key);
    let ledger_index_key = runtime_gateway_redis_ledger_index_key(ledger_key);
    let ledger_id_index_key = runtime_gateway_redis_ledger_id_index_key(ledger_key);
    for delta in deltas {
        let usage_hash_key = runtime_gateway_redis_usage_hash_key(usage_key, &delta.key_name);
        let spend_microusd = delta.estimated_cost_microusd.unwrap_or_default();
        let entry = runtime_gateway_billing_ledger_entry_from_delta(delta);
        let entry_id = runtime_gateway_redis_ledger_entry_id(&entry);
        let entry_key = runtime_gateway_redis_ledger_entry_key(ledger_key, &entry_id);
        let call_index_key =
            runtime_gateway_redis_ledger_call_index_key(ledger_key, &entry.call_id);
        let payload = serde_json::to_string(&entry)?;
        let _: i32 = redis::cmd("EVAL")
            .arg(RUNTIME_GATEWAY_REDIS_USAGE_APPLY_SCRIPT)
            .arg(6)
            .arg(&usage_index_key)
            .arg(&usage_hash_key)
            .arg(&ledger_index_key)
            .arg(&call_index_key)
            .arg(&entry_key)
            .arg(&ledger_id_index_key)
            .arg(&delta.key_name)
            .arg(runtime_gateway_sqlite_u64_to_i64(delta.minute_epoch))
            .arg(runtime_gateway_sqlite_u64_to_i64(delta.reserved_tokens))
            .arg(runtime_gateway_sqlite_u64_to_i64(spend_microusd))
            .arg(&entry_id)
            .arg(payload)
            .query(&mut conn)?;
    }
    Ok(())
}

pub(super) fn runtime_gateway_sqlite_usage_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage> {
    Ok(runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
        minute_epoch: runtime_gateway_sqlite_usage_u64(row, 1)?,
        requests_this_minute: runtime_gateway_sqlite_usage_u64(row, 2)?,
        tokens_this_minute: runtime_gateway_sqlite_usage_u64(row, 3)?,
        requests_total: runtime_gateway_sqlite_usage_u64(row, 4)?,
        spend_microusd: runtime_gateway_sqlite_usage_u64(row, 5)?,
    })
}

fn runtime_gateway_sqlite_usage_u64(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Integer, Box::new(err))
    })
}

pub(super) fn runtime_gateway_postgres_usage_from_row(
    row: &postgres::Row,
) -> Result<runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage> {
    Ok(runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
        minute_epoch: runtime_gateway_postgres_usage_u64(row, 1)?,
        requests_this_minute: runtime_gateway_postgres_usage_u64(row, 2)?,
        tokens_this_minute: runtime_gateway_postgres_usage_u64(row, 3)?,
        requests_total: runtime_gateway_postgres_usage_u64(row, 4)?,
        spend_microusd: runtime_gateway_postgres_usage_u64(row, 5)?,
    })
}

fn runtime_gateway_postgres_usage_u64(row: &postgres::Row, index: usize) -> Result<u64> {
    let value: i64 = row.get(index);
    u64::try_from(value).context("gateway usage counter is negative")
}

#[cfg(test)]
#[path = "local_rewrite_gateway_usage_backend_tests.rs"]
mod tests;
