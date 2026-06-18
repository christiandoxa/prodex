use anyhow::Result;
use redis::Commands;

use super::local_rewrite_gateway_backend_connection::{
    runtime_gateway_redis_connection, runtime_gateway_redis_with_lock,
};
use super::local_rewrite_gateway_ledger_types::{
    RuntimeGatewayBillingLedgerEntry, runtime_gateway_apply_response_to_ledger_entry,
    runtime_gateway_billing_ledger_entry_from_delta,
};
use super::local_rewrite_gateway_usage_backend::RuntimeGatewayVirtualKeyUsageDelta;
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;

pub(super) fn runtime_gateway_redis_append_ledger_deltas<G>(
    url: &str,
    ledger_key: &str,
    lock_key: &str,
    token_generator: G,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()>
where
    G: FnOnce() -> Result<String>,
{
    runtime_gateway_redis_with_lock(url, lock_key, token_generator, |conn| {
        let mut entries =
            runtime_gateway_redis_ledger_load_from_connection(conn, ledger_key, usize::MAX)?;
        for delta in deltas {
            let next = runtime_gateway_billing_ledger_entry_from_delta(delta);
            let exists = entries.iter().any(|entry| {
                entry.request == next.request
                    && entry.key_name.eq_ignore_ascii_case(&next.key_name)
                    && entry.phase == next.phase
            });
            if !exists {
                entries.push(next);
            }
        }
        runtime_gateway_redis_replace_ledger_entries(conn, ledger_key, &entries)?;
        Ok(())
    })
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
    let start = if limit == usize::MAX {
        0
    } else {
        -(i64::try_from(limit).unwrap_or(i64::MAX))
    };
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

fn runtime_gateway_redis_replace_ledger_entries(
    conn: &mut redis::Connection,
    ledger_key: &str,
    entries: &[RuntimeGatewayBillingLedgerEntry],
) -> Result<()> {
    let _: () = conn.del(ledger_key)?;
    for entry in entries {
        let payload = serde_json::to_string(entry)?;
        let _: () = conn.rpush(ledger_key, payload)?;
    }
    Ok(())
}

pub(super) fn runtime_gateway_redis_ledger_reconcile_response<G>(
    url: &str,
    ledger_key: &str,
    lock_key: &str,
    token_generator: G,
    event: &RuntimeProviderGatewaySpendEvent,
    reconciled_at_epoch: u64,
) -> Result<bool>
where
    G: FnOnce() -> Result<String>,
{
    runtime_gateway_redis_with_lock(url, lock_key, token_generator, |conn| {
        let mut entries =
            runtime_gateway_redis_ledger_load_from_connection(conn, ledger_key, usize::MAX)?;
        let mut changed = false;
        for entry in &mut entries {
            if entry.request != event.request || entry.phase != "request" {
                continue;
            }
            runtime_gateway_apply_response_to_ledger_entry(entry, event, reconciled_at_epoch);
            changed = true;
        }
        if changed {
            runtime_gateway_redis_replace_ledger_entries(conn, ledger_key, &entries)?;
        }
        Ok(changed)
    })
}
