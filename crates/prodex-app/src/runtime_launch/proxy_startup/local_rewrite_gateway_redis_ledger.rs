use anyhow::Result;
use redis::Commands;

use super::local_rewrite_gateway_backend_connection::runtime_gateway_redis_connection;
use super::local_rewrite_gateway_ledger_types::{
    RuntimeGatewayBillingLedgerEntry, runtime_gateway_apply_response_to_ledger_entry,
    runtime_gateway_billing_ledger_entry_from_delta, runtime_gateway_billing_ledger_entry_identity,
};
use super::local_rewrite_gateway_usage_backend::RuntimeGatewayVirtualKeyUsageDelta;
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;

const RUNTIME_GATEWAY_REDIS_LEDGER_LOAD_LIMIT_MAX: usize = 100_000;

pub(super) fn runtime_gateway_redis_append_ledger_deltas<G>(
    url: &str,
    ledger_key: &str,
    _lock_key: &str,
    _token_generator: G,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()>
where
    G: FnOnce() -> Result<String>,
{
    let mut conn = runtime_gateway_redis_connection(url)?;
    let index_key = runtime_gateway_redis_ledger_index_key(ledger_key);
    let script = r#"
        if redis.call('SETNX', KEYS[2], ARGV[2]) == 1 then
            redis.call('RPUSH', KEYS[1], ARGV[1])
            redis.call('SADD', KEYS[3], ARGV[1])
            return 1
        end
        return 0
        "#;
    for delta in deltas {
        let entry = runtime_gateway_billing_ledger_entry_from_delta(delta);
        let entry_id = runtime_gateway_redis_ledger_entry_id(&entry);
        let entry_key = runtime_gateway_redis_ledger_entry_key(ledger_key, &entry_id);
        let call_index_key =
            runtime_gateway_redis_ledger_call_index_key(ledger_key, &entry.call_id);
        let payload = serde_json::to_string(&entry)?;
        let _: i32 = redis::cmd("EVAL")
            .arg(script)
            .arg(3)
            .arg(&index_key)
            .arg(&entry_key)
            .arg(&call_index_key)
            .arg(&entry_id)
            .arg(payload)
            .query(&mut conn)?;
    }
    Ok(())
}

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
    let index_key = runtime_gateway_redis_ledger_index_key(ledger_key);
    let start = -(i64::try_from(limit).unwrap_or(i64::MAX));
    let ids: Vec<String> = redis::cmd("LRANGE")
        .arg(&index_key)
        .arg(start)
        .arg(-1)
        .query(conn)?;
    if ids.is_empty() {
        return runtime_gateway_redis_legacy_ledger_load_from_connection(conn, ledger_key, limit);
    }

    let mut entries = Vec::new();
    for id in ids {
        let payload: Option<String> =
            conn.get(runtime_gateway_redis_ledger_entry_key(ledger_key, &id))?;
        if let Some(payload) = payload
            && let Ok(entry) = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(&payload)
        {
            entries.push(entry);
        }
    }
    Ok(entries)
}

fn runtime_gateway_redis_legacy_ledger_load_from_connection(
    conn: &mut redis::Connection,
    ledger_key: &str,
    limit: usize,
) -> Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let limit = runtime_gateway_redis_ledger_load_limit(limit);
    if limit == 0 {
        return Ok(Vec::new());
    }
    let start = -(i64::try_from(limit).unwrap_or(i64::MAX));
    let payloads: Vec<String> = redis::cmd("LRANGE")
        .arg(ledger_key)
        .arg(start)
        .arg(-1)
        .query(conn)?;
    let mut entries = Vec::new();
    for payload in payloads {
        if let Ok(entry) = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(&payload) {
            entries.push(entry);
        }
    }
    Ok(entries)
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
    let ids: Vec<String> = conn.smembers(runtime_gateway_redis_ledger_call_index_key(
        ledger_key,
        &event.call_id,
    ))?;
    let mut changed = false;
    for id in ids {
        let entry_key = runtime_gateway_redis_ledger_entry_key(ledger_key, &id);
        let payload: Option<String> = conn.get(&entry_key)?;
        let Some(payload) = payload else {
            continue;
        };
        let Ok(mut entry) = serde_json::from_str::<RuntimeGatewayBillingLedgerEntry>(&payload)
        else {
            continue;
        };
        if entry.call_id != event.call_id || entry.phase != "request" {
            continue;
        }
        runtime_gateway_apply_response_to_ledger_entry(&mut entry, event, reconciled_at_epoch);
        let _: () = conn.set(entry_key, serde_json::to_string(&entry)?)?;
        changed = true;
    }
    Ok(changed)
}

fn runtime_gateway_redis_ledger_index_key(ledger_key: &str) -> String {
    format!("{ledger_key}:entries")
}

fn runtime_gateway_redis_ledger_call_index_key(ledger_key: &str, call_id: &str) -> String {
    format!("{ledger_key}:call:{call_id}")
}

fn runtime_gateway_redis_ledger_entry_key(ledger_key: &str, entry_id: &str) -> String {
    format!("{ledger_key}:entry:{entry_id}")
}

fn runtime_gateway_redis_ledger_entry_id(entry: &RuntimeGatewayBillingLedgerEntry) -> String {
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
    fn redis_ledger_backend_does_not_rewrite_whole_ledger_under_lock() {
        let source = include_str!("local_rewrite_gateway_redis_ledger.rs");
        let with_lock = ["runtime_gateway_redis", "_with_lock"].join("");
        let replace_all = ["runtime_gateway_redis_replace", "_ledger_entries"].join("");
        let delete_all = ["conn.del", "(ledger_key"].join("");
        let unbounded_limit = ["limit == usize", "::MAX"].join("");

        assert!(!source.contains(&with_lock));
        assert!(!source.contains(&replace_all));
        assert!(!source.contains(&delete_all));
        assert!(!source.contains(&unbounded_limit));
        assert!(source.contains("SETNX"));
        assert!(source.contains("RPUSH"));
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
}
