use std::collections::BTreeSet;

use anyhow::{Context, Result};
use redis::Commands;

use super::local_rewrite_gateway_backend_connection::runtime_gateway_redis_connection;
#[cfg(test)]
use super::local_rewrite_gateway_ledger_types::runtime_gateway_billing_ledger_entry_from_delta;
use super::local_rewrite_gateway_ledger_types::{
    RuntimeGatewayBillingLedgerEntry, runtime_gateway_apply_response_to_ledger_entry,
    runtime_gateway_billing_ledger_entry_identity,
};
#[cfg(test)]
use super::local_rewrite_gateway_usage_backend::RuntimeGatewayVirtualKeyUsageDelta;
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;

const RUNTIME_GATEWAY_REDIS_LEDGER_LOAD_LIMIT_MAX: usize = 100_000;
const RUNTIME_GATEWAY_REDIS_LEDGER_ID_INDEX_MARKER_VALUE: &str = "1";
const RUNTIME_GATEWAY_REDIS_LEDGER_ID_INDEX_BACKFILL_SCRIPT: &str = r#"
        local function valid_type(key, expected)
            local actual = redis.call('TYPE', key).ok
            return actual == 'none' or actual == expected
        end
        if not valid_type(KEYS[1], 'list')
            or not valid_type(KEYS[2], 'set')
            or not valid_type(KEYS[3], 'string') then
            return redis.error_reply('WRONGTYPE gateway ledger ID-index key')
        end
        for index = 1, #ARGV do
            redis.call('SADD', KEYS[2], ARGV[index])
        end
        local list_count = redis.call('LLEN', KEYS[1])
        local set_count = redis.call('SCARD', KEYS[2])
        if list_count ~= set_count or list_count ~= #ARGV then
            return redis.error_reply('gateway ledger list and ID index counts differ')
        end
        redis.call('SET', KEYS[3], '1')
        return list_count
        "#;
const RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_FINALIZE_SCRIPT: &str = r#"
        local actual_type = redis.call('TYPE', KEYS[1]).ok
        if actual_type ~= 'none' and actual_type ~= 'list' then
            return redis.error_reply('WRONGTYPE legacy gateway ledger key')
        end
        local actual_count = redis.call('LLEN', KEYS[1])
        if actual_count == 0 then
            return 0
        end
        if actual_count ~= tonumber(ARGV[1]) then
            return redis.error_reply('legacy gateway ledger changed during migration')
        end
        redis.call('DEL', KEYS[1])
        return 1
        "#;
const RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_MIGRATE_SCRIPT: &str = r#"
        local function valid_type(key, expected)
            local actual = redis.call('TYPE', key).ok
            return actual == 'none' or actual == expected
        end
        if not valid_type(KEYS[1], 'list')
            or not valid_type(KEYS[2], 'string')
            or not valid_type(KEYS[3], 'set')
            or not valid_type(KEYS[4], 'set') then
            return redis.error_reply('WRONGTYPE gateway ledger key')
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
        local entry_exists = redis.call('EXISTS', KEYS[2])
        local call_member = redis.call('SISMEMBER', KEYS[3], ARGV[1])
        local global_member = redis.call('SISMEMBER', KEYS[4], ARGV[1])
        if entry_exists == 1 then
            if call_member ~= 1 or global_member ~= 1
                or ledger_identity(redis.call('GET', KEYS[2])) ~= ARGV[1] then
                return redis.error_reply('gateway ledger index is inconsistent')
            end
            return 0
        end
        if call_member ~= 0 or global_member ~= 0 then
            return redis.error_reply('gateway ledger index is inconsistent')
        end
        redis.call('SET', KEYS[2], ARGV[2])
        redis.call('LPUSH', KEYS[1], ARGV[1])
        redis.call('SADD', KEYS[3], ARGV[1])
        redis.call('SADD', KEYS[4], ARGV[1])
        return 1
        "#;

pub(super) fn runtime_gateway_redis_ledger_load(
    url: &str,
    ledger_key: &str,
    limit: usize,
) -> Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let mut conn = runtime_gateway_redis_connection(url)?;
    runtime_gateway_redis_ledger_load_from_connection(&mut conn, ledger_key, limit)
}

fn runtime_gateway_redis_ledger_load_from_connection(
    conn: &mut redis::Connection,
    ledger_key: &str,
    limit: usize,
) -> Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let limit = runtime_gateway_redis_ledger_load_limit(limit);
    if limit == 0 {
        return Ok(Vec::new());
    }
    runtime_gateway_redis_migrate_legacy_ledger_from_connection(conn, ledger_key)?;
    let index_key = runtime_gateway_redis_ledger_index_key(ledger_key);
    let start = -(i64::try_from(limit).unwrap_or(i64::MAX));
    let ids: Vec<String> = redis::cmd("LRANGE")
        .arg(&index_key)
        .arg(start)
        .arg(-1)
        .query(conn)?;
    let id_index_key = runtime_gateway_redis_ledger_id_index_key(ledger_key);
    if !ids.is_empty() {
        let mut membership = redis::pipe();
        for id in &ids {
            membership.cmd("SISMEMBER").arg(&id_index_key).arg(id);
        }
        let indexed: Vec<bool> = membership.query(conn)?;
        if indexed.iter().any(|member| !member) {
            anyhow::bail!("gateway redis ledger list contains an unindexed entry");
        }
    }

    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            anyhow::bail!("gateway redis ledger index contains a duplicate entry");
        }
        let payload = conn
            .get::<_, Option<String>>(runtime_gateway_redis_ledger_entry_key(ledger_key, &id))?
            .ok_or_else(|| {
                anyhow::anyhow!("gateway redis ledger index references a missing entry")
            })?;
        let entry = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(&payload)
            .context("failed to parse gateway redis ledger entry")?;
        if runtime_gateway_redis_ledger_entry_id(&entry) != id {
            anyhow::bail!("gateway redis ledger entry identity does not match its index");
        }
        entries.push(entry);
    }
    Ok(entries)
}

pub(super) fn runtime_gateway_redis_migrate_legacy_ledger_from_connection(
    conn: &mut redis::Connection,
    ledger_key: &str,
) -> Result<()> {
    runtime_gateway_redis_ensure_ledger_id_index_from_connection(conn, ledger_key)?;
    let legacy_count: usize = conn.llen(ledger_key)?;
    if legacy_count > RUNTIME_GATEWAY_REDIS_LEDGER_LOAD_LIMIT_MAX {
        anyhow::bail!("legacy gateway redis ledger exceeds the migration limit");
    }
    if legacy_count == 0 {
        return Ok(());
    }
    let payloads: Vec<String> = redis::cmd("LRANGE")
        .arg(ledger_key)
        .arg(0)
        .arg(-1)
        .query(conn)?;
    if payloads.len() > RUNTIME_GATEWAY_REDIS_LEDGER_LOAD_LIMIT_MAX {
        anyhow::bail!("legacy gateway redis ledger exceeds the migration limit");
    }
    let index_key = runtime_gateway_redis_ledger_index_key(ledger_key);
    let id_index_key = runtime_gateway_redis_ledger_id_index_key(ledger_key);
    let mut migrations = Vec::with_capacity(payloads.len());
    for payload in payloads {
        let entry = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(&payload)
            .context("failed to parse legacy gateway redis ledger entry")?;
        let entry_id = runtime_gateway_redis_ledger_entry_id(&entry);
        let entry_key = runtime_gateway_redis_ledger_entry_key(ledger_key, &entry_id);
        let call_index_key =
            runtime_gateway_redis_ledger_call_index_key(ledger_key, &entry.call_id);
        migrations.push((entry_id, entry_key, call_index_key, payload));
    }
    if !migrations.is_empty() {
        let mut pipe = redis::pipe();
        for (entry_id, entry_key, call_index_key, payload) in migrations.into_iter().rev() {
            pipe.cmd("EVAL")
                .arg(RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_MIGRATE_SCRIPT)
                .arg(4)
                .arg(&index_key)
                .arg(entry_key)
                .arg(call_index_key)
                .arg(&id_index_key)
                .arg(entry_id)
                .arg(payload);
        }
        pipe.query::<Vec<i32>>(conn)?;
    }
    let _: i32 = redis::cmd("EVAL")
        .arg(RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_FINALIZE_SCRIPT)
        .arg(1)
        .arg(ledger_key)
        .arg(legacy_count)
        .query(conn)?;
    Ok(())
}

fn runtime_gateway_redis_ensure_ledger_id_index_from_connection(
    conn: &mut redis::Connection,
    ledger_key: &str,
) -> Result<()> {
    let marker_key = runtime_gateway_redis_ledger_id_index_marker_key(ledger_key);
    let marker: Option<String> = conn.get(&marker_key)?;
    if let Some(marker) = marker {
        if marker != RUNTIME_GATEWAY_REDIS_LEDGER_ID_INDEX_MARKER_VALUE {
            anyhow::bail!("gateway redis ledger ID-index marker is invalid");
        }
        return runtime_gateway_redis_validate_ledger_id_index_counts(conn, ledger_key);
    }

    let index_key = runtime_gateway_redis_ledger_index_key(ledger_key);
    let index_count: usize = conn.llen(&index_key)?;
    if index_count > RUNTIME_GATEWAY_REDIS_LEDGER_LOAD_LIMIT_MAX {
        anyhow::bail!("gateway redis ledger index exceeds the backfill limit");
    }
    let ids: Vec<String> = conn.lrange(&index_key, 0, -1)?;
    if ids.len() != index_count || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        anyhow::bail!("gateway redis ledger index contains duplicate or unstable entries");
    }

    if !ids.is_empty() {
        let entry_keys = ids
            .iter()
            .map(|id| runtime_gateway_redis_ledger_entry_key(ledger_key, id))
            .collect::<Vec<_>>();
        let payloads: Vec<Option<String>> = redis::cmd("MGET").arg(&entry_keys).query(conn)?;
        let mut call_indexes = Vec::with_capacity(ids.len());
        for (id, payload) in ids.iter().zip(payloads) {
            let payload = payload.ok_or_else(|| {
                anyhow::anyhow!("gateway redis ledger index references a missing entry")
            })?;
            let entry = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(&payload)
                .context("failed to parse gateway redis ledger entry during ID-index backfill")?;
            if runtime_gateway_redis_ledger_entry_id(&entry) != id.as_str() {
                anyhow::bail!("gateway redis ledger entry identity does not match its index");
            }
            call_indexes.push(runtime_gateway_redis_ledger_call_index_key(
                ledger_key,
                &entry.call_id,
            ));
        }
        let mut membership = redis::pipe();
        for (call_index, id) in call_indexes.iter().zip(&ids) {
            membership.cmd("SISMEMBER").arg(call_index).arg(id);
        }
        let call_members: Vec<bool> = membership.query(conn)?;
        if call_members.iter().any(|member| !member) {
            anyhow::bail!("gateway redis ledger entry is missing from its call index");
        }
    }

    let _: usize = redis::cmd("EVAL")
        .arg(RUNTIME_GATEWAY_REDIS_LEDGER_ID_INDEX_BACKFILL_SCRIPT)
        .arg(3)
        .arg(&index_key)
        .arg(runtime_gateway_redis_ledger_id_index_key(ledger_key))
        .arg(&marker_key)
        .arg(&ids)
        .query(conn)?;
    runtime_gateway_redis_validate_ledger_id_index_counts(conn, ledger_key)
}

fn runtime_gateway_redis_validate_ledger_id_index_counts(
    conn: &mut redis::Connection,
    ledger_key: &str,
) -> Result<()> {
    let list_count: usize = conn.llen(runtime_gateway_redis_ledger_index_key(ledger_key))?;
    let set_count: usize = conn.scard(runtime_gateway_redis_ledger_id_index_key(ledger_key))?;
    if list_count != set_count {
        anyhow::bail!("gateway redis ledger list and ID index counts differ");
    }
    Ok(())
}

pub(super) fn runtime_gateway_redis_ledger_reconcile_response<G>(
    url: &str,
    ledger_key: &str,
    _lock_key: &str,
    _token_generator: G,
    event: &RuntimeProviderGatewaySpendEvent,
    reconciled_at_epoch: u64,
) -> Result<bool>
where
    G: FnOnce() -> Result<String>,
{
    let mut conn = runtime_gateway_redis_connection(url)?;
    runtime_gateway_redis_migrate_legacy_ledger_from_connection(&mut conn, ledger_key)?;
    let ids: Vec<String> = conn.smembers(runtime_gateway_redis_ledger_call_index_key(
        ledger_key,
        &event.call_id,
    ))?;
    let mut changed = false;
    for id in ids {
        if !conn.sismember(runtime_gateway_redis_ledger_id_index_key(ledger_key), &id)? {
            anyhow::bail!("gateway redis ledger call index references an unindexed entry");
        }
        let entry_key = runtime_gateway_redis_ledger_entry_key(ledger_key, &id);
        let payload: Option<String> = conn.get(&entry_key)?;
        let payload = payload.ok_or_else(|| {
            anyhow::anyhow!("gateway redis ledger call index references a missing entry")
        })?;
        let mut entry = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(&payload)
            .context("failed to parse gateway redis ledger entry during reconciliation")?;
        if runtime_gateway_redis_ledger_entry_id(&entry) != id {
            anyhow::bail!("gateway redis ledger entry identity does not match its call index");
        }
        if entry.call_id != event.call_id {
            anyhow::bail!("gateway redis ledger call index references another call");
        }
        if entry.phase != "request" {
            continue;
        }
        runtime_gateway_apply_response_to_ledger_entry(&mut entry, event, reconciled_at_epoch);
        let _: () = conn.set(entry_key, serde_json::to_string(&entry)?)?;
        changed = true;
    }
    Ok(changed)
}

pub(super) fn runtime_gateway_redis_ledger_index_key(ledger_key: &str) -> String {
    format!("{ledger_key}:entries")
}

pub(super) fn runtime_gateway_redis_ledger_call_index_key(
    ledger_key: &str,
    call_id: &str,
) -> String {
    format!("{ledger_key}:call:{call_id}")
}

pub(super) fn runtime_gateway_redis_ledger_entry_key(ledger_key: &str, entry_id: &str) -> String {
    format!("{ledger_key}:entry:{entry_id}")
}

pub(super) fn runtime_gateway_redis_ledger_id_index_key(ledger_key: &str) -> String {
    format!("{ledger_key}:entry_ids")
}

pub(super) fn runtime_gateway_redis_ledger_id_index_marker_key(ledger_key: &str) -> String {
    format!("{ledger_key}:entry_ids_migrated_v1")
}

pub(super) fn runtime_gateway_redis_ledger_entry_id(
    entry: &RuntimeGatewayBillingLedgerEntry,
) -> String {
    runtime_gateway_billing_ledger_entry_identity(entry)
}

fn runtime_gateway_redis_ledger_load_limit(limit: usize) -> usize {
    limit.min(RUNTIME_GATEWAY_REDIS_LEDGER_LOAD_LIMIT_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger_entry() -> RuntimeGatewayBillingLedgerEntry {
        runtime_gateway_billing_ledger_entry_from_delta(&RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 42,
            typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            call_id: format!("prodex-{}", prodex_domain::CallId::new()),
            key_name: "Team-A".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5".to_string(),
            minute_epoch: 10,
            input_tokens: 100,
            reserved_tokens: 100,
            estimated_cost_microusd: Some(250_000),
            created_at_epoch: 20,
        })
    }

    #[test]
    fn redis_ledger_keys_are_entry_scoped() {
        let entry = ledger_entry();
        let entry_id = runtime_gateway_redis_ledger_entry_id(&entry);
        let mut duplicate = entry.clone();
        duplicate.input_tokens = 200;

        assert_eq!(entry_id, runtime_gateway_redis_ledger_entry_id(&duplicate));
        assert_eq!(
            runtime_gateway_redis_ledger_index_key("prodex:gateway:billing_ledger"),
            "prodex:gateway:billing_ledger:entries"
        );
        assert_eq!(
            runtime_gateway_redis_ledger_entry_key("prodex:gateway:billing_ledger", &entry_id),
            format!("prodex:gateway:billing_ledger:entry:{entry_id}")
        );
        assert_eq!(
            runtime_gateway_redis_ledger_call_index_key(
                "prodex:gateway:billing_ledger",
                &entry.call_id
            ),
            format!("prodex:gateway:billing_ledger:call:{}", entry.call_id)
        );
    }

    #[test]
    fn redis_ledger_entry_ids_do_not_fold_case_or_delimiters() {
        let mut entry = ledger_entry();
        entry.call_id = "call:with-delimiter".to_string();
        entry.key_name = "Team-A".to_string();
        let entry_id = runtime_gateway_redis_ledger_entry_id(&entry);

        entry.key_name = "team-a".to_string();
        assert_ne!(entry_id, runtime_gateway_redis_ledger_entry_id(&entry));

        entry.call_id = "call".to_string();
        entry.key_name = "with-delimiter:Team-A".to_string();
        assert_ne!(entry_id, runtime_gateway_redis_ledger_entry_id(&entry));
    }

    #[test]
    fn redis_ledger_load_limit_bounds_unlimited_requests() {
        assert_eq!(runtime_gateway_redis_ledger_load_limit(0), 0);
        assert_eq!(runtime_gateway_redis_ledger_load_limit(1000), 1000);
        assert_eq!(
            runtime_gateway_redis_ledger_load_limit(usize::MAX),
            RUNTIME_GATEWAY_REDIS_LEDGER_LOAD_LIMIT_MAX
        );
    }

    #[test]
    fn legacy_ledger_migration_prepends_without_scanning_new_entries() {
        assert!(!RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_MIGRATE_SCRIPT.contains("LPOS"));
        assert!(RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_MIGRATE_SCRIPT.contains("LPUSH"));
        assert!(RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_MIGRATE_SCRIPT.contains("SISMEMBER', KEYS[4]"));
        assert!(RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_MIGRATE_SCRIPT.contains("SADD', KEYS[4]"));
        assert!(RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_FINALIZE_SCRIPT.contains("LLEN"));
        assert!(RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_FINALIZE_SCRIPT.contains("tonumber(ARGV[1])"));
    }

    #[test]
    #[ignore = "requires PRODEX_TEST_REDIS_URL"]
    fn redis_legacy_ledger_finalize_preserves_concurrent_appends() {
        let url = std::env::var("PRODEX_TEST_REDIS_URL")
            .expect("PRODEX_TEST_REDIS_URL must point to the test Redis instance");
        let ledger_key = format!(
            "prodex:test:gateway:legacy-ledger-finalize:{}",
            prodex_domain::RequestId::new()
        );
        let mut conn = runtime_gateway_redis_connection(&url).unwrap();
        let _: usize = conn.rpush(&ledger_key, "first").unwrap();
        let expected_count: usize = conn.llen(&ledger_key).unwrap();
        let _: usize = conn.rpush(&ledger_key, "concurrent").unwrap();

        let err = redis::cmd("EVAL")
            .arg(RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_FINALIZE_SCRIPT)
            .arg(1)
            .arg(&ledger_key)
            .arg(expected_count)
            .query::<i32>(&mut conn)
            .unwrap_err();
        assert!(
            err.to_string().contains("changed during migration"),
            "{err:?}"
        );
        assert_eq!(conn.llen::<_, usize>(&ledger_key).unwrap(), 2);

        let deleted: i32 = redis::cmd("EVAL")
            .arg(RUNTIME_GATEWAY_REDIS_LEGACY_LEDGER_FINALIZE_SCRIPT)
            .arg(1)
            .arg(&ledger_key)
            .arg(2)
            .query(&mut conn)
            .unwrap();
        assert_eq!(deleted, 1);
        assert!(!conn.exists::<_, bool>(&ledger_key).unwrap());
    }
}
