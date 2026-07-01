use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use redis::Commands;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_open, runtime_gateway_redis_connection, runtime_gateway_sqlite_open,
};
use super::local_rewrite_gateway_ledger_types::runtime_gateway_billing_ledger_entry_from_delta;
use super::local_rewrite_gateway_redis_ledger::{
    runtime_gateway_redis_append_ledger_deltas, runtime_gateway_redis_ledger_load,
};
use super::local_rewrite_gateway_sqlite_utils::runtime_gateway_sqlite_i64_to_u64;
use super::local_rewrite_gateway_sqlite_utils::{
    runtime_gateway_sqlite_optional_u64_to_i64, runtime_gateway_sqlite_u64_to_i64,
};

#[derive(Clone, Debug)]
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
    pub(super) estimated_cost_microusd: Option<u64>,
    pub(super) created_at_epoch: u64,
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
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let mut client = runtime_gateway_postgres_open(url)?;
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
        usage.insert(row.get(0), runtime_gateway_postgres_usage_from_row(&row));
    }
    Ok(usage)
}

pub(super) fn runtime_gateway_redis_usage_load(
    url: &str,
    redis_key: &str,
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let mut conn = runtime_gateway_redis_connection(url)?;
    let index_key = runtime_gateway_redis_usage_index_key(redis_key);
    let names: Vec<String> = conn.smembers(&index_key)?;
    if names.is_empty() {
        let payload: Option<String> = conn.get(redis_key)?;
        let Some(payload) = payload else {
            return Ok(BTreeMap::new());
        };
        return serde_json::from_str::<
            BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
        >(&payload)
        .context("failed to parse legacy gateway redis virtual key usage");
    }

    let mut usage = BTreeMap::new();
    for name in names {
        let hash_key = runtime_gateway_redis_usage_hash_key(redis_key, &name);
        let fields: BTreeMap<String, String> = conn.hgetall(&hash_key)?;
        if fields.is_empty() {
            continue;
        }
        usage.insert(name, runtime_gateway_redis_usage_from_hash(&fields));
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
) -> runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
    runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
        minute_epoch: runtime_gateway_redis_hash_u64(fields, "minute_epoch"),
        requests_this_minute: runtime_gateway_redis_hash_u64(fields, "requests_this_minute"),
        tokens_this_minute: runtime_gateway_redis_hash_u64(fields, "tokens_this_minute"),
        requests_total: runtime_gateway_redis_hash_u64(fields, "requests_total"),
        spend_microusd: runtime_gateway_redis_hash_u64(fields, "spend_microusd"),
    }
}

fn runtime_gateway_redis_hash_u64(fields: &BTreeMap<String, String>, name: &str) -> u64 {
    fields
        .get(name)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default()
}

pub(super) fn runtime_gateway_sqlite_usage_apply_deltas(
    path: &Path,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()> {
    let mut conn = runtime_gateway_sqlite_open(path)?;
    let ledger_has_scope_columns = runtime_gateway_sqlite_ledger_has_scope_columns(&conn)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for delta in deltas {
        let ledger = runtime_gateway_billing_ledger_entry_from_delta(delta);
        let inserted = if ledger_has_scope_columns {
            tx.execute(
                r#"
                INSERT OR IGNORE INTO prodex_gateway_billing_ledger (
                    phase, request_id, typed_request_id, call_id, key_name,
                    tenant_id, team_id, project_id, user_id, budget_id, model, minute_epoch,
                    input_tokens, estimated_cost_microusd, created_at_epoch
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
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
                    runtime_gateway_sqlite_optional_u64_to_i64(ledger.estimated_cost_microusd),
                    runtime_gateway_sqlite_u64_to_i64(ledger.created_at_epoch),
                ],
            )?
        } else {
            tx.execute(
                r#"
                INSERT OR IGNORE INTO prodex_gateway_billing_ledger (
                    phase, request_id, call_id, key_name, model, minute_epoch,
                    input_tokens, estimated_cost_microusd, created_at_epoch
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    ledger.phase,
                    runtime_gateway_sqlite_u64_to_i64(ledger.request),
                    ledger.call_id,
                    ledger.key_name,
                    ledger.model,
                    runtime_gateway_sqlite_u64_to_i64(ledger.minute_epoch),
                    runtime_gateway_sqlite_u64_to_i64(ledger.input_tokens),
                    runtime_gateway_sqlite_optional_u64_to_i64(ledger.estimated_cost_microusd),
                    runtime_gateway_sqlite_u64_to_i64(ledger.created_at_epoch),
                ],
            )?
        };
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
        let admission = runtime_proxy_crate::RuntimeGatewayVirtualKeyAdmission {
            key_name: delta.key_name.clone(),
            model: None,
            input_tokens: delta.input_tokens,
            estimated_cost_microusd: delta.estimated_cost_microusd,
        };
        runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
            &mut usage,
            &admission,
            delta.minute_epoch,
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
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()> {
    let mut client = runtime_gateway_postgres_open(url)?;
    let ledger_has_scope_columns = runtime_gateway_postgres_ledger_has_scope_columns(&mut client)?;
    let mut tx = client.transaction()?;
    for delta in deltas {
        let ledger = runtime_gateway_billing_ledger_entry_from_delta(delta);
        let inserted = if ledger_has_scope_columns {
            tx.execute(
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
                    &runtime_gateway_sqlite_optional_u64_to_i64(ledger.estimated_cost_microusd),
                    &runtime_gateway_sqlite_u64_to_i64(ledger.created_at_epoch),
                ],
            )?
        } else {
            tx.execute(
                RUNTIME_GATEWAY_POSTGRES_LEGACY_LEDGER_INSERT_SQL,
                &[
                    &ledger.phase,
                    &runtime_gateway_sqlite_u64_to_i64(ledger.request),
                    &ledger.call_id,
                    &ledger.key_name,
                    &ledger.model,
                    &runtime_gateway_sqlite_u64_to_i64(ledger.minute_epoch),
                    &runtime_gateway_sqlite_u64_to_i64(ledger.input_tokens),
                    &runtime_gateway_sqlite_optional_u64_to_i64(ledger.estimated_cost_microusd),
                    &runtime_gateway_sqlite_u64_to_i64(ledger.created_at_epoch),
                ],
            )?
        };
        if inserted == 0 {
            continue;
        }
        tx.execute(
            RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL,
            &[
                &delta.key_name,
                &runtime_gateway_sqlite_u64_to_i64(delta.minute_epoch),
                &runtime_gateway_sqlite_u64_to_i64(delta.input_tokens),
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
                input_tokens, estimated_cost_microusd, created_at_epoch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT(call_id, key_name, phase) DO NOTHING
            "#;

const RUNTIME_GATEWAY_POSTGRES_LEGACY_LEDGER_INSERT_SQL: &str = r#"
            INSERT INTO prodex_gateway_billing_ledger (
                phase, request_id, call_id, key_name, model, minute_epoch,
                input_tokens, estimated_cost_microusd, created_at_epoch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT(call_id, key_name, phase) DO NOTHING
            "#;

fn runtime_gateway_sqlite_ledger_has_scope_columns(conn: &Connection) -> Result<bool> {
    let count: i64 = conn.query_row(
        r#"
        SELECT COUNT(*)
        FROM pragma_table_info('prodex_gateway_billing_ledger')
        WHERE name IN (
            'typed_request_id', 'tenant_id', 'team_id', 'project_id', 'user_id', 'budget_id'
        )
        "#,
        [],
        |row| row.get(0),
    )?;
    Ok(count == 6)
}

fn runtime_gateway_postgres_ledger_has_scope_columns(
    client: &mut postgres::Client,
) -> Result<bool> {
    let count: i64 = client
        .query_one(
            r#"
            SELECT COUNT(*)::BIGINT
            FROM information_schema.columns
            WHERE table_name = 'prodex_gateway_billing_ledger'
              AND column_name IN (
                  'typed_request_id', 'tenant_id', 'team_id', 'project_id', 'user_id', 'budget_id'
              )
            "#,
            &[],
        )?
        .get(0);
    Ok(count == 6)
}

pub(super) fn runtime_gateway_redis_usage_apply_deltas<G>(
    url: &str,
    usage_key: &str,
    _usage_lock_key: &str,
    ledger_key: &str,
    ledger_lock_key: &str,
    token_generator: G,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()>
where
    G: FnOnce() -> Result<String>,
{
    let mut seen_requests = runtime_gateway_redis_ledger_load(url, ledger_key, usize::MAX)?
        .into_iter()
        .filter(|entry| entry.phase == "request")
        .map(|entry| (entry.request, entry.key_name.to_ascii_lowercase()))
        .collect::<std::collections::BTreeSet<_>>();
    let unique_deltas = deltas
        .iter()
        .filter(|&delta| {
            seen_requests.insert((delta.request_id, delta.key_name.to_ascii_lowercase()))
        })
        .cloned()
        .collect::<Vec<_>>();
    if unique_deltas.is_empty() {
        return Ok(());
    }
    let mut conn = runtime_gateway_redis_connection(url)?;
    let script = r#"
        redis.call('SADD', KEYS[1], ARGV[1])
        local current_minute = redis.call('HGET', KEYS[2], 'minute_epoch')
        if current_minute ~= false and tonumber(current_minute) ~= tonumber(ARGV[2]) then
            redis.call('HSET', KEYS[2], 'requests_this_minute', 0, 'tokens_this_minute', 0)
        end
        redis.call('HSET', KEYS[2], 'minute_epoch', ARGV[2])
        redis.call('HINCRBY', KEYS[2], 'requests_this_minute', 1)
        redis.call('HINCRBY', KEYS[2], 'tokens_this_minute', ARGV[3])
        redis.call('HINCRBY', KEYS[2], 'requests_total', 1)
        redis.call('HINCRBY', KEYS[2], 'spend_microusd', ARGV[4])
        return 1
        "#;
    let index_key = runtime_gateway_redis_usage_index_key(usage_key);
    for delta in &unique_deltas {
        let hash_key = runtime_gateway_redis_usage_hash_key(usage_key, &delta.key_name);
        let spend_microusd = delta.estimated_cost_microusd.unwrap_or_default();
        let _: i32 = redis::cmd("EVAL")
            .arg(script)
            .arg(2)
            .arg(&index_key)
            .arg(&hash_key)
            .arg(&delta.key_name)
            .arg(runtime_gateway_sqlite_u64_to_i64(delta.minute_epoch))
            .arg(runtime_gateway_sqlite_u64_to_i64(delta.input_tokens))
            .arg(runtime_gateway_sqlite_u64_to_i64(spend_microusd))
            .query(&mut conn)?;
    }
    runtime_gateway_redis_append_ledger_deltas(
        url,
        ledger_key,
        ledger_lock_key,
        token_generator,
        &unique_deltas,
    )?;
    Ok(())
}

pub(super) fn runtime_gateway_sqlite_usage_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage> {
    Ok(runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
        minute_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(1)?),
        requests_this_minute: runtime_gateway_sqlite_i64_to_u64(row.get(2)?),
        tokens_this_minute: runtime_gateway_sqlite_i64_to_u64(row.get(3)?),
        requests_total: runtime_gateway_sqlite_i64_to_u64(row.get(4)?),
        spend_microusd: runtime_gateway_sqlite_i64_to_u64(row.get(5)?),
    })
}

pub(super) fn runtime_gateway_postgres_usage_from_row(
    row: &postgres::Row,
) -> runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
    runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
        minute_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(1)),
        requests_this_minute: runtime_gateway_sqlite_i64_to_u64(row.get(2)),
        tokens_this_minute: runtime_gateway_sqlite_i64_to_u64(row.get(3)),
        requests_total: runtime_gateway_sqlite_i64_to_u64(row.get(4)),
        spend_microusd: runtime_gateway_sqlite_i64_to_u64(row.get(5)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::local_rewrite_gateway_sqlite_utils::runtime_gateway_sqlite_u64_to_i64;
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("prodex-gateway-usage-{name}-{stamp}"))
    }

    #[test]
    fn sqlite_usage_load_reads_usage_rows() {
        let root = temp_dir("sqlite");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("state.sqlite");
        let conn = runtime_gateway_sqlite_open(&path).unwrap();
        conn.execute(
            r#"
            INSERT INTO prodex_gateway_virtual_key_usage (
                key_name, minute_epoch, requests_this_minute, tokens_this_minute,
                requests_total, spend_microusd
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            rusqlite::params![
                "alpha",
                runtime_gateway_sqlite_u64_to_i64(10),
                runtime_gateway_sqlite_u64_to_i64(1),
                runtime_gateway_sqlite_u64_to_i64(20),
                runtime_gateway_sqlite_u64_to_i64(2),
                runtime_gateway_sqlite_u64_to_i64(300),
            ],
        )
        .unwrap();

        let usage = runtime_gateway_sqlite_usage_load(&path).unwrap();
        assert_eq!(usage["alpha"].minute_epoch, 10);
        assert_eq!(usage["alpha"].spend_microusd, 300);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn redis_usage_hash_helpers_round_trip_usage_fields() {
        let mut fields = BTreeMap::new();
        fields.insert("minute_epoch".to_string(), "42".to_string());
        fields.insert("requests_this_minute".to_string(), "3".to_string());
        fields.insert("tokens_this_minute".to_string(), "144".to_string());
        fields.insert("requests_total".to_string(), "9".to_string());
        fields.insert("spend_microusd".to_string(), "1700".to_string());

        let usage = runtime_gateway_redis_usage_from_hash(&fields);

        assert_eq!(usage.minute_epoch, 42);
        assert_eq!(usage.requests_this_minute, 3);
        assert_eq!(usage.tokens_this_minute, 144);
        assert_eq!(usage.requests_total, 9);
        assert_eq!(usage.spend_microusd, 1700);
    }

    #[test]
    fn redis_usage_hash_keys_are_per_virtual_key() {
        assert_eq!(
            runtime_gateway_redis_usage_index_key("prodex:gateway:virtual_key_usage"),
            "prodex:gateway:virtual_key_usage:keys"
        );
        assert_eq!(
            runtime_gateway_redis_usage_hash_key("prodex:gateway:virtual_key_usage", "team-a"),
            "prodex:gateway:virtual_key_usage:key:team-a"
        );
    }

    #[test]
    fn redis_usage_backend_does_not_write_whole_usage_json_blob() {
        let source = include_str!("local_rewrite_gateway_usage_backend.rs");
        let set_blob = ["conn.set", "(redis_key"].join("");
        let whole_usage_json = ["serde_json::to_string", "(usage"].join("");
        let hincrby = ["HINC", "RBY"].join("");

        assert!(!source.contains(&set_blob));
        assert!(!source.contains(&whole_usage_json));
        assert!(source.contains(&hincrby));
    }

    #[test]
    fn postgres_usage_upsert_increments_counters_atomically() {
        assert!(RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL.contains("ON CONFLICT(key_name)"));
        assert!(RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL.contains("requests_total + 1"));
        assert!(
            RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL
                .contains("spend_microusd + EXCLUDED.spend_microusd")
        );
        assert!(!RUNTIME_GATEWAY_POSTGRES_USAGE_UPSERT_SQL.contains("FOR UPDATE"));
    }

    #[test]
    fn postgres_ledger_insert_conflict_target_uses_call_id() {
        for column in [
            "typed_request_id",
            "tenant_id",
            "team_id",
            "project_id",
            "user_id",
            "budget_id",
        ] {
            assert!(RUNTIME_GATEWAY_POSTGRES_LEDGER_INSERT_SQL.contains(column));
        }
        assert!(
            RUNTIME_GATEWAY_POSTGRES_LEDGER_INSERT_SQL
                .contains("ON CONFLICT(call_id, key_name, phase) DO NOTHING")
        );
        assert!(
            !RUNTIME_GATEWAY_POSTGRES_LEDGER_INSERT_SQL
                .contains("ON CONFLICT(request_id, key_name, phase)")
        );
    }
}
