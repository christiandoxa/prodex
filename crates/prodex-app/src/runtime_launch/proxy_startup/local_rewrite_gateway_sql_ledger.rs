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
    let sql = r#"
        SELECT phase, request_id, typed_request_id, call_id, key_name, tenant_id,
               team_id, project_id, user_id, budget_id, model, minute_epoch,
               input_tokens, estimated_cost_microusd, created_at_epoch,
               response_status, response_bytes, output_tokens, final_cost_microusd,
               final_cost_usd, reconciled_at_epoch
        FROM prodex_gateway_billing_ledger
        ORDER BY id DESC
        LIMIT ?1
        "#;
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(
        params![runtime_gateway_sqlite_u64_to_i64(limit as u64)],
        |row| {
            let estimated_cost_microusd = runtime_gateway_sqlite_optional_i64_to_u64(row.get(13)?);
            Ok(RuntimeGatewayBillingLedgerEntry {
                object: "gateway.billing_ledger_entry".to_string(),
                phase: row.get(0)?,
                request_id: row.get(2)?,
                request: runtime_gateway_sqlite_i64_to_u64(row.get(1)?),
                call_id: row.get(3)?,
                key_name: row.get(4)?,
                tenant_id: row.get(5)?,
                team_id: row.get(6)?,
                project_id: row.get(7)?,
                user_id: row.get(8)?,
                budget_id: row.get(9)?,
                model: row.get(10)?,
                minute_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(11)?),
                input_tokens: runtime_gateway_sqlite_i64_to_u64(row.get(12)?),
                estimated_cost_microusd,
                estimated_cost_usd: estimated_cost_microusd.map(microusd_to_usd),
                created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(14)?),
                response_status: runtime_gateway_sqlite_optional_i64_to_u64(row.get(15)?)
                    .and_then(|value| u16::try_from(value).ok()),
                response_bytes: runtime_gateway_sqlite_optional_i64_to_u64(row.get(16)?),
                output_tokens: runtime_gateway_sqlite_optional_i64_to_u64(row.get(17)?),
                final_cost_microusd: runtime_gateway_sqlite_optional_i64_to_u64(row.get(18)?),
                final_cost_usd: row.get(19)?,
                reconciled_at_epoch: runtime_gateway_sqlite_optional_i64_to_u64(row.get(20)?),
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
    let sql = r#"
        SELECT phase, request_id, typed_request_id, call_id, key_name, tenant_id,
               team_id, project_id, user_id, budget_id, model, minute_epoch,
               input_tokens, estimated_cost_microusd, created_at_epoch,
               response_status, response_bytes, output_tokens, final_cost_microusd,
               final_cost_usd, reconciled_at_epoch
        FROM prodex_gateway_billing_ledger
        ORDER BY id DESC
        LIMIT $1
        "#;
    let rows = client.query(sql, &[&runtime_gateway_sqlite_u64_to_i64(limit as u64)])?;
    let mut entries = Vec::new();
    for row in rows {
        let estimated_cost_microusd = runtime_gateway_sqlite_optional_i64_to_u64(row.get(13));
        entries.push(RuntimeGatewayBillingLedgerEntry {
            object: "gateway.billing_ledger_entry".to_string(),
            phase: row.get(0),
            request_id: row.get(2),
            request: runtime_gateway_sqlite_i64_to_u64(row.get(1)),
            call_id: row.get(3),
            key_name: row.get(4),
            tenant_id: row.get(5),
            team_id: row.get(6),
            project_id: row.get(7),
            user_id: row.get(8),
            budget_id: row.get(9),
            model: row.get(10),
            minute_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(11)),
            input_tokens: runtime_gateway_sqlite_i64_to_u64(row.get(12)),
            estimated_cost_microusd,
            estimated_cost_usd: estimated_cost_microusd.map(microusd_to_usd),
            created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(14)),
            response_status: runtime_gateway_sqlite_optional_i64_to_u64(row.get(15))
                .and_then(|value| u16::try_from(value).ok()),
            response_bytes: runtime_gateway_sqlite_optional_i64_to_u64(row.get(16)),
            output_tokens: runtime_gateway_sqlite_optional_i64_to_u64(row.get(17)),
            final_cost_microusd: runtime_gateway_sqlite_optional_i64_to_u64(row.get(18)),
            final_cost_usd: row.get(19),
            reconciled_at_epoch: runtime_gateway_sqlite_optional_i64_to_u64(row.get(20)),
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
    let Some(key_name) = event.key_name.as_deref() else {
        return Ok(false);
    };
    let changed = conn.execute(
        r#"
        UPDATE prodex_gateway_billing_ledger
        SET response_status = ?1,
            response_bytes = ?2,
            output_tokens = ?3,
            final_cost_microusd = ?4,
            final_cost_usd = ?5,
            reconciled_at_epoch = ?6
        WHERE call_id = ?7
          AND key_name = ?8
          AND tenant_id IS ?9
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
            event.call_id.clone(),
            key_name,
            event.tenant_id.clone(),
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
    let Some(key_name) = event.key_name.as_deref() else {
        return Ok(false);
    };
    let changed = client.execute(
        r#"
        UPDATE prodex_gateway_billing_ledger
        SET response_status = $1,
            response_bytes = $2,
            output_tokens = $3,
            final_cost_microusd = $4,
            final_cost_usd = $5,
            reconciled_at_epoch = $6
        WHERE call_id = $7
          AND key_name = $8
          AND tenant_id IS NOT DISTINCT FROM $9
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
            &event.call_id,
            &key_name,
            &event.tenant_id,
        ],
    )?;
    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::super::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_create_current_schema_for_tests;
    use super::super::local_rewrite_gateway_ledger_types::runtime_gateway_billing_ledger_entry_from_delta;
    use super::super::local_rewrite_gateway_usage_backend::{
        RuntimeGatewayVirtualKeyUsageDelta, runtime_gateway_sqlite_usage_apply_deltas,
    };
    use super::*;

    fn temp_sqlite_ledger_path(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-sql-ledger-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("state.sqlite");
        (root, path)
    }

    fn create_legacy_sqlite_ledger_schema(path: &std::path::Path) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE prodex_gateway_schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at_epoch INTEGER NOT NULL
            );
            INSERT INTO prodex_gateway_schema_migrations (version, applied_at_epoch)
            VALUES (1, 1);
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
    fn sqlite_ledger_load_reads_recent_entries_in_oldest_first_order() {
        let (root, path) = temp_sqlite_ledger_path("load");
        runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
        let conn = runtime_gateway_sqlite_open(&path).unwrap();
        for request_id in [1_u64, 2] {
            let ledger = runtime_gateway_billing_ledger_entry_from_delta(
                &RuntimeGatewayVirtualKeyUsageDelta {
                    request_id,
                    typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
                    call_id: format!("prodex-{}", prodex_domain::CallId::new()),
                    key_name: "alpha".to_string(),
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

    #[test]
    fn sqlite_ledger_round_trips_typed_request_id_and_scope_snapshot() {
        let (root, path) = temp_sqlite_ledger_path("typed-scope");
        runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
        let typed_request_id = format!("prodex-{}", prodex_domain::RequestId::new());
        runtime_gateway_sqlite_usage_apply_deltas(
            &path,
            &[RuntimeGatewayVirtualKeyUsageDelta {
                request_id: 7,
                typed_request_id: typed_request_id.clone(),
                call_id: format!("prodex-{}", prodex_domain::CallId::new()),
                key_name: "alpha".to_string(),
                tenant_id: Some("tenant-a".to_string()),
                team_id: Some("team-a".to_string()),
                project_id: Some("project-a".to_string()),
                user_id: Some("user-a".to_string()),
                budget_id: Some("budget-a".to_string()),
                model: "gpt-5".to_string(),
                minute_epoch: 10,
                input_tokens: 100,
                reserved_tokens: 100,
                estimated_cost_microusd: Some(250_000),
                created_at_epoch: 20,
            }],
        )
        .unwrap();

        let entries = runtime_gateway_sqlite_ledger_load(&path, 1).unwrap();
        let entry = entries.first().unwrap();

        assert_eq!(entry.request_id.as_deref(), Some(typed_request_id.as_str()));
        assert_eq!(entry.tenant_id.as_deref(), Some("tenant-a"));
        assert_eq!(entry.team_id.as_deref(), Some("team-a"));
        assert_eq!(entry.project_id.as_deref(), Some("project-a"));
        assert_eq!(entry.user_id.as_deref(), Some("user-a"));
        assert_eq!(entry.budget_id.as_deref(), Some("budget-a"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sqlite_legacy_ledger_schema_is_upgraded_by_migrator_before_usage_deltas() {
        let (root, path) = temp_sqlite_ledger_path("legacy-schema");
        create_legacy_sqlite_ledger_schema(&path);
        runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();

        runtime_gateway_sqlite_usage_apply_deltas(
            &path,
            &[RuntimeGatewayVirtualKeyUsageDelta {
                request_id: 8,
                typed_request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
                call_id: format!("prodex-{}", prodex_domain::CallId::new()),
                key_name: "alpha".to_string(),
                tenant_id: Some("tenant-a".to_string()),
                team_id: Some("team-a".to_string()),
                project_id: None,
                user_id: None,
                budget_id: None,
                model: "gpt-5".to_string(),
                minute_epoch: 10,
                input_tokens: 100,
                reserved_tokens: 100,
                estimated_cost_microusd: Some(250_000),
                created_at_epoch: 20,
            }],
        )
        .unwrap();

        let entries = runtime_gateway_sqlite_ledger_load(&path, 1).unwrap();
        let entry = entries.first().unwrap();

        assert_eq!(entry.request, 8);
        assert!(entry.request_id.is_some());
        assert_eq!(entry.tenant_id.as_deref(), Some("tenant-a"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sqlite_ledger_reconcile_matches_call_id_and_key_scope() {
        let (root, path) = temp_sqlite_ledger_path("call-id-reconcile");
        runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
        let conn = runtime_gateway_sqlite_open(&path).unwrap();
        for (call_id, model) in [("call-a", "gpt-a"), ("call-b", "gpt-b")] {
            conn.execute(
                r#"
                INSERT INTO prodex_gateway_billing_ledger (
                    phase, request_id, call_id, key_name, model, minute_epoch,
                    input_tokens, created_at_epoch
                )
                VALUES ('request', 7, ?1, 'team-a', ?2, 10, 1, 20)
                "#,
                rusqlite::params![call_id, model],
            )
            .unwrap();
        }

        runtime_gateway_sqlite_ledger_reconcile_response(
            &path,
            &RuntimeProviderGatewaySpendEvent {
                event: "gateway_spend",
                phase: "response",
                request: 7,
                key_name: Some("team-a".to_string()),
                tenant_id: None,
                request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
                legacy_request_sequence: 7,
                call_id: "call-b".to_string(),
                provider: "openai".to_string(),
                path: "/v1/responses".to_string(),
                model: "gpt-b".to_string(),
                status: 200,
                elapsed_ms: 12,
                request_bytes: 3,
                response_bytes: Some(44),
                input_tokens: Some(1),
                output_tokens: Some(2),
                cost_usd: Some(0.25),
                sink: "runtime-log".to_string(),
            },
            30,
        )
        .unwrap();

        let call_a_status: Option<i64> = conn
            .query_row(
                "SELECT response_status FROM prodex_gateway_billing_ledger WHERE call_id = 'call-a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let call_b_status: Option<i64> = conn
            .query_row(
                "SELECT response_status FROM prodex_gateway_billing_ledger WHERE call_id = 'call-b'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(call_a_status, None);
        assert_eq!(call_b_status, Some(200));

        std::fs::remove_dir_all(root).unwrap();
    }
}
