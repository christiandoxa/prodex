use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use redis::Commands;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, types::Type};

use super::local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_open, runtime_gateway_redis_connection,
    runtime_gateway_redis_with_lock, runtime_gateway_sqlite_open,
};
use super::local_rewrite_gateway_ledger_types::runtime_gateway_billing_ledger_entry_from_delta;
use super::local_rewrite_gateway_redis_ledger::{
    runtime_gateway_redis_append_ledger_deltas, runtime_gateway_redis_ledger_load,
};
use super::local_rewrite_gateway_sqlite_utils::{
    runtime_gateway_sqlite_optional_u64_to_i64, runtime_gateway_sqlite_u64_to_i64,
};

#[derive(Clone, Debug)]
pub(super) struct RuntimeGatewayVirtualKeyUsageDelta {
    pub(super) request_id: u64,
    pub(super) key_name: String,
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
        usage.insert(row.get(0), runtime_gateway_postgres_usage_from_row(&row)?);
    }
    Ok(usage)
}

pub(super) fn runtime_gateway_redis_usage_load(
    url: &str,
    redis_key: &str,
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    let mut conn = runtime_gateway_redis_connection(url)?;
    let payload: Option<String> = conn.get(redis_key)?;
    let Some(payload) = payload else {
        return Ok(BTreeMap::new());
    };
    serde_json::from_str::<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>>(
        &payload,
    )
    .context("failed to parse gateway redis virtual key usage")
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
    let mut tx = client.transaction()?;
    for delta in deltas {
        let ledger = runtime_gateway_billing_ledger_entry_from_delta(delta);
        let inserted = tx.execute(
            r#"
            INSERT INTO prodex_gateway_billing_ledger (
                phase, request_id, call_id, key_name, model, minute_epoch,
                input_tokens, estimated_cost_microusd, created_at_epoch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT(request_id, key_name, phase) DO NOTHING
            "#,
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
        )?;
        if inserted == 0 {
            continue;
        }
        let mut usage = tx
            .query_opt(
                r#"
                SELECT key_name, minute_epoch, requests_this_minute, tokens_this_minute,
                       requests_total, spend_microusd
                FROM prodex_gateway_virtual_key_usage
                WHERE lower(key_name) = lower($1)
                FOR UPDATE
                "#,
                &[&delta.key_name],
            )?
            .map(|row| runtime_gateway_postgres_usage_from_row(&row))
            .transpose()?
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
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT(key_name) DO UPDATE SET
                minute_epoch = EXCLUDED.minute_epoch,
                requests_this_minute = EXCLUDED.requests_this_minute,
                tokens_this_minute = EXCLUDED.tokens_this_minute,
                requests_total = EXCLUDED.requests_total,
                spend_microusd = EXCLUDED.spend_microusd
            "#,
            &[
                &delta.key_name,
                &runtime_gateway_sqlite_u64_to_i64(usage.minute_epoch),
                &runtime_gateway_sqlite_u64_to_i64(usage.requests_this_minute),
                &runtime_gateway_sqlite_u64_to_i64(usage.tokens_this_minute),
                &runtime_gateway_sqlite_u64_to_i64(usage.requests_total),
                &runtime_gateway_sqlite_u64_to_i64(usage.spend_microusd),
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub(super) fn runtime_gateway_redis_usage_apply_deltas<G>(
    url: &str,
    usage_key: &str,
    usage_lock_key: &str,
    ledger_key: &str,
    ledger_lock_key: &str,
    token_generator: G,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> Result<()>
where
    G: Fn() -> Result<String> + Copy,
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
    runtime_gateway_redis_with_lock(url, usage_lock_key, token_generator, |conn| {
        let payload: Option<String> = conn.get(usage_key)?;
        let mut usage = match payload {
            Some(payload) => serde_json::from_str::<
                BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
            >(&payload)
            .context("failed to parse gateway redis virtual key usage")?,
            None => BTreeMap::new(),
        };
        for delta in &unique_deltas {
            let entry = usage.entry(delta.key_name.clone()).or_default();
            let admission = runtime_proxy_crate::RuntimeGatewayVirtualKeyAdmission {
                key_name: delta.key_name.clone(),
                model: None,
                input_tokens: delta.input_tokens,
                estimated_cost_microusd: delta.estimated_cost_microusd,
            };
            runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
                entry,
                &admission,
                delta.minute_epoch,
            );
        }
        let payload = serde_json::to_string(&usage)?;
        let _: () = conn.set(usage_key, payload)?;
        Ok(())
    })?;
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
    fn sqlite_usage_load_rejects_negative_usage_rows() {
        let root = temp_dir("sqlite-negative");
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
            rusqlite::params!["alpha", 10_i64, 1_i64, 20_i64, -1_i64, 300_i64],
        )
        .unwrap();

        assert!(runtime_gateway_sqlite_usage_load(&path).is_err());

        std::fs::remove_dir_all(root).unwrap();
    }
}
