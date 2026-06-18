use std::path::Path;

use anyhow::Result;
use prodex_provider_core::microusd_to_usd;
use rusqlite::params;

use super::local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_open, runtime_gateway_sqlite_open,
};
use super::local_rewrite_gateway_ledger_types::{
    RuntimeGatewayBillingLedgerEntry, runtime_gateway_usd_to_microusd,
};
use super::local_rewrite_gateway_sqlite_utils::{
    runtime_gateway_sqlite_i64_to_u64, runtime_gateway_sqlite_optional_i64_to_u64,
    runtime_gateway_sqlite_optional_u64_to_i64, runtime_gateway_sqlite_u64_to_i64,
};
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;

pub(super) fn runtime_gateway_sqlite_ledger_load(
    path: &Path,
    limit: usize,
) -> Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let conn = runtime_gateway_sqlite_open(path)?;
    let mut stmt = conn.prepare(
        r#"
        SELECT phase, request_id, call_id, key_name, model, minute_epoch,
               input_tokens, estimated_cost_microusd, created_at_epoch,
               response_status, response_bytes, output_tokens, final_cost_microusd,
               final_cost_usd, reconciled_at_epoch
        FROM prodex_gateway_billing_ledger
        ORDER BY id DESC
        LIMIT ?1
        "#,
    )?;
    let rows = stmt.query_map(
        params![runtime_gateway_sqlite_u64_to_i64(limit as u64)],
        |row| {
            let estimated_cost_microusd = runtime_gateway_sqlite_optional_i64_to_u64(row.get(7)?);
            Ok(RuntimeGatewayBillingLedgerEntry {
                object: "gateway.billing_ledger_entry".to_string(),
                phase: row.get(0)?,
                request: runtime_gateway_sqlite_i64_to_u64(row.get(1)?),
                call_id: row.get(2)?,
                key_name: row.get(3)?,
                model: row.get(4)?,
                minute_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(5)?),
                input_tokens: runtime_gateway_sqlite_i64_to_u64(row.get(6)?),
                estimated_cost_microusd,
                estimated_cost_usd: estimated_cost_microusd.map(microusd_to_usd),
                created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(8)?),
                response_status: runtime_gateway_sqlite_optional_i64_to_u64(row.get(9)?)
                    .and_then(|value| u16::try_from(value).ok()),
                response_bytes: runtime_gateway_sqlite_optional_i64_to_u64(row.get(10)?),
                output_tokens: runtime_gateway_sqlite_optional_i64_to_u64(row.get(11)?),
                final_cost_microusd: runtime_gateway_sqlite_optional_i64_to_u64(row.get(12)?),
                final_cost_usd: row.get(13)?,
                reconciled_at_epoch: runtime_gateway_sqlite_optional_i64_to_u64(row.get(14)?),
            })
        },
    )?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    entries.reverse();
    Ok(entries)
}

pub(super) fn runtime_gateway_postgres_ledger_load(
    url: &str,
    limit: usize,
) -> Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    let mut client = runtime_gateway_postgres_open(url)?;
    let rows = client.query(
        r#"
        SELECT phase, request_id, call_id, key_name, model, minute_epoch,
               input_tokens, estimated_cost_microusd, created_at_epoch,
               response_status, response_bytes, output_tokens, final_cost_microusd,
               final_cost_usd, reconciled_at_epoch
        FROM prodex_gateway_billing_ledger
        ORDER BY id DESC
        LIMIT $1
        "#,
        &[&runtime_gateway_sqlite_u64_to_i64(limit as u64)],
    )?;
    let mut entries = Vec::new();
    for row in rows {
        let estimated_cost_microusd = runtime_gateway_sqlite_optional_i64_to_u64(row.get(7));
        entries.push(RuntimeGatewayBillingLedgerEntry {
            object: "gateway.billing_ledger_entry".to_string(),
            phase: row.get(0),
            request: runtime_gateway_sqlite_i64_to_u64(row.get(1)),
            call_id: row.get(2),
            key_name: row.get(3),
            model: row.get(4),
            minute_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(5)),
            input_tokens: runtime_gateway_sqlite_i64_to_u64(row.get(6)),
            estimated_cost_microusd,
            estimated_cost_usd: estimated_cost_microusd.map(microusd_to_usd),
            created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(8)),
            response_status: runtime_gateway_sqlite_optional_i64_to_u64(row.get(9))
                .and_then(|value| u16::try_from(value).ok()),
            response_bytes: runtime_gateway_sqlite_optional_i64_to_u64(row.get(10)),
            output_tokens: runtime_gateway_sqlite_optional_i64_to_u64(row.get(11)),
            final_cost_microusd: runtime_gateway_sqlite_optional_i64_to_u64(row.get(12)),
            final_cost_usd: row.get(13),
            reconciled_at_epoch: runtime_gateway_sqlite_optional_i64_to_u64(row.get(14)),
        });
    }
    entries.reverse();
    Ok(entries)
}

pub(super) fn runtime_gateway_sqlite_ledger_reconcile_response(
    path: &Path,
    event: &RuntimeProviderGatewaySpendEvent,
    reconciled_at_epoch: u64,
) -> Result<bool> {
    let conn = runtime_gateway_sqlite_open(path)?;
    let changed = conn.execute(
        r#"
        UPDATE prodex_gateway_billing_ledger
        SET response_status = ?1,
            response_bytes = ?2,
            output_tokens = ?3,
            final_cost_microusd = ?4,
            final_cost_usd = ?5,
            reconciled_at_epoch = ?6
        WHERE request_id = ?7
          AND phase = 'request'
        "#,
        params![
            i64::from(event.status),
            runtime_gateway_sqlite_optional_u64_to_i64(
                event.response_bytes.map(|value| value as u64)
            ),
            runtime_gateway_sqlite_optional_u64_to_i64(event.output_tokens),
            runtime_gateway_sqlite_optional_u64_to_i64(runtime_gateway_usd_to_microusd(
                event.cost_usd
            )),
            event.cost_usd,
            runtime_gateway_sqlite_u64_to_i64(reconciled_at_epoch),
            runtime_gateway_sqlite_u64_to_i64(event.request),
        ],
    )?;
    Ok(changed > 0)
}

pub(super) fn runtime_gateway_postgres_ledger_reconcile_response(
    url: &str,
    event: &RuntimeProviderGatewaySpendEvent,
    reconciled_at_epoch: u64,
) -> Result<bool> {
    let mut client = runtime_gateway_postgres_open(url)?;
    let changed = client.execute(
        r#"
        UPDATE prodex_gateway_billing_ledger
        SET response_status = $1,
            response_bytes = $2,
            output_tokens = $3,
            final_cost_microusd = $4,
            final_cost_usd = $5,
            reconciled_at_epoch = $6
        WHERE request_id = $7
          AND phase = 'request'
        "#,
        &[
            &i64::from(event.status),
            &runtime_gateway_sqlite_optional_u64_to_i64(
                event.response_bytes.map(|value| value as u64),
            ),
            &runtime_gateway_sqlite_optional_u64_to_i64(event.output_tokens),
            &runtime_gateway_sqlite_optional_u64_to_i64(runtime_gateway_usd_to_microusd(
                event.cost_usd,
            )),
            &event.cost_usd,
            &runtime_gateway_sqlite_u64_to_i64(reconciled_at_epoch),
            &runtime_gateway_sqlite_u64_to_i64(event.request),
        ],
    )?;
    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::super::local_rewrite_gateway_ledger_types::runtime_gateway_billing_ledger_entry_from_delta;
    use super::super::local_rewrite_gateway_usage_backend::RuntimeGatewayVirtualKeyUsageDelta;
    use super::*;

    #[test]
    fn sqlite_ledger_load_reads_recent_entries_in_oldest_first_order() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-sql-ledger-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("state.sqlite");
        let conn = runtime_gateway_sqlite_open(&path).unwrap();
        for request_id in [1_u64, 2] {
            let ledger = runtime_gateway_billing_ledger_entry_from_delta(
                &RuntimeGatewayVirtualKeyUsageDelta {
                    request_id,
                    key_name: "alpha".to_string(),
                    model: "gpt-5".to_string(),
                    minute_epoch: 10,
                    input_tokens: 100,
                    estimated_cost_microusd: Some(250_000),
                    created_at_epoch: 20 + request_id,
                },
            );
            conn.execute(
                r#"
                INSERT INTO prodex_gateway_billing_ledger (
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
            )
            .unwrap();
        }

        let entries = runtime_gateway_sqlite_ledger_load(&path, 2).unwrap();
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.request)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
