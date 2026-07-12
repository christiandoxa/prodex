use super::local_rewrite::{
    RUNTIME_GATEWAY_REDIS_LEDGER_KEY, RUNTIME_GATEWAY_REDIS_LEDGER_LOCK,
    RuntimeGatewayDurableReservationState,
};
use super::local_rewrite_application_data_plane::{
    RuntimeGatewayApplicationReconciliationInput, runtime_gateway_application_usage_reconciliation,
};
use super::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_open;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_file_ledger::{
    runtime_gateway_file_ledger_load, runtime_gateway_file_ledger_reconcile_response,
};
use super::local_rewrite_gateway_ledger_types::RuntimeGatewayBillingLedgerEntry;
use super::local_rewrite_gateway_reconciliation_runtime::{
    runtime_gateway_durable_actual_usage, runtime_gateway_postgres_load_durable_reservation_state,
    runtime_gateway_postgres_reconcile_usage,
};
use super::local_rewrite_gateway_redis_ledger::{
    runtime_gateway_redis_ledger_load, runtime_gateway_redis_ledger_reconcile_response,
};
use super::local_rewrite_gateway_sql_ledger::{
    runtime_gateway_postgres_ledger_load, runtime_gateway_postgres_ledger_reconcile_response,
    runtime_gateway_sqlite_ledger_load, runtime_gateway_sqlite_ledger_reconcile_response,
};
use super::local_rewrite_gateway_sqlite_utils::runtime_gateway_sqlite_u64_to_i64;
use super::local_rewrite_gateway_util::{
    runtime_gateway_generate_virtual_key_token, runtime_gateway_unix_epoch_millis,
    runtime_gateway_unix_epoch_seconds,
};
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;
use super::*;
use prodex_domain::{RequestId, UsageAmount};
use rusqlite::OptionalExtension;
use std::collections::BTreeMap;
use std::sync::Arc;

pub(super) fn runtime_gateway_billing_ledger_load(
    state_store: &RuntimeGatewayStateStore,
    limit: usize,
) -> std::io::Result<Vec<RuntimeGatewayBillingLedgerEntry>> {
    match state_store {
        RuntimeGatewayStateStore::File { ledger_path, .. } => {
            runtime_gateway_file_ledger_load(ledger_path, limit)
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            runtime_gateway_sqlite_ledger_load(path, limit).map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Postgres { url, tls, .. } => {
            runtime_gateway_postgres_ledger_load(url, tls, limit).map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_redis_ledger_load(url, RUNTIME_GATEWAY_REDIS_LEDGER_KEY, limit)
                .map_err(std::io::Error::other)
        }
    }
}

pub(super) fn runtime_gateway_durable_reconcile_response(
    runtime_shared: &RuntimeRotationProxyShared,
    state_store: &RuntimeGatewayStateStore,
    postgres_repository: Option<&prodex_storage_postgres_runtime::PostgresRepository>,
    durable_reservations: &Arc<Mutex<BTreeMap<u64, RuntimeGatewayDurableReservationState>>>,
    event: &RuntimeProviderGatewaySpendEvent,
) -> std::io::Result<()> {
    let state = durable_reservations
        .lock()
        .ok()
        .and_then(|reservations| reservations.get(&event.request).cloned());
    let state = match (state, state_store) {
        (None, RuntimeGatewayStateStore::Postgres { .. }) => {
            runtime_gateway_postgres_load_durable_reservation_state(
                runtime_shared,
                postgres_repository,
                event,
            )?
        }
        (state, _) => state,
    };
    let Some(state) = state else {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_durable_reservation_state_missing",
                [
                    runtime_proxy_log_field("request", event.request.to_string()),
                    runtime_proxy_log_field("backend", state_store.label()),
                ],
            ),
        );
        return Ok(());
    };
    let (actual, clamped) = runtime_gateway_durable_actual_usage(&state.record, event);
    if clamped {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_durable_actual_clamped",
                [
                    runtime_proxy_log_field("request", event.request.to_string()),
                    runtime_proxy_log_field("backend", state_store.label()),
                    runtime_proxy_log_field(
                        "reserved_tokens",
                        state.record.reserved.tokens.to_string(),
                    ),
                    runtime_proxy_log_field(
                        "actual_tokens",
                        event
                            .input_tokens
                            .unwrap_or_default()
                            .saturating_add(event.output_tokens.unwrap_or_default())
                            .to_string(),
                    ),
                ],
            ),
        );
    }
    let planned = runtime_gateway_application_usage_reconciliation(
        RuntimeGatewayApplicationReconciliationInput {
            state_store,
            storage_key: state.storage_key,
            record: state.record,
            actual,
            event,
        },
    )
    .map_err(std::io::Error::other)?;
    if planned
        .application
        .gateway
        .reconciliation
        .reconciliation
        .reason
        != planned.command.reason
    {
        return Err(std::io::Error::other(
            "application reconciliation plan mismatch",
        ));
    }
    match (state_store, planned.application.storage) {
        (
            RuntimeGatewayStateStore::Sqlite { path },
            prodex_application::ApplicationUsageReconciliationStoragePlan::Sqlite(storage),
        ) => runtime_gateway_sqlite_reconcile_usage(
            path,
            &storage,
            &planned.command.record,
            planned.command.actual,
        )
        .map_err(std::io::Error::other),
        (
            RuntimeGatewayStateStore::Postgres { .. },
            prodex_application::ApplicationUsageReconciliationStoragePlan::Postgres(storage),
        ) => runtime_gateway_postgres_reconcile_usage(
            runtime_shared,
            postgres_repository,
            &storage,
            planned.command,
        ),
        (RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. }, _) => {
            Ok(())
        }
        _ => Err(std::io::Error::other(
            "application reconciliation storage mismatch",
        )),
    }
}

fn runtime_gateway_reconciliation_storage_scope(
    storage_key: prodex_storage::TenantStorageKey,
) -> String {
    storage_key.storage_scope()
}

fn runtime_gateway_sqlite_reconcile_usage(
    path: &std::path::Path,
    plan: &prodex_storage_sqlite::SqliteUsageReconciliationSqlPlan,
    record: &prodex_domain::ReservationRecord,
    actual: UsageAmount,
) -> anyhow::Result<()> {
    let mut conn = runtime_gateway_sqlite_open(path)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let tenant_id = plan.tenant_id.to_string();
    let reservation_id = record.reservation_id.to_string();
    let call_id = record.call_id.to_string();
    let committed: Option<i64> = tx
        .query_row(
            "SELECT committed_at_unix_ms FROM prodex_reservations WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3",
            rusqlite::params![tenant_id, reservation_id, call_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    if committed.is_some() {
        tx.commit()?;
        return Ok(());
    }
    let updated = runtime_gateway_unix_epoch_millis();
    let storage_scope = runtime_gateway_reconciliation_storage_scope(plan.storage_key);
    let released_tokens = record.reserved.tokens.saturating_sub(actual.tokens);
    let released_cost_micros = record
        .reserved
        .cost_micros
        .saturating_sub(actual.cost_micros);
    let changed = tx.execute(
        r#"
        UPDATE prodex_budget_counters
        SET reserved_tokens = reserved_tokens - ?4,
            reserved_cost_micros = reserved_cost_micros - ?5,
            committed_tokens = committed_tokens + ?6,
            committed_cost_micros = committed_cost_micros + ?7,
            updated_at_unix_ms = ?8
        WHERE tenant_id = ?1
          AND storage_scope = ?9
          AND reserved_tokens >= ?4
          AND reserved_cost_micros >= ?5
          AND EXISTS (
              SELECT 1
              FROM prodex_reservations
              WHERE tenant_id = ?1
                AND reservation_id = ?2
                AND call_id = ?3
                AND committed_at_unix_ms IS NULL
          )
        "#,
        rusqlite::params![
            tenant_id,
            reservation_id,
            call_id,
            runtime_gateway_sqlite_u64_to_i64(record.reserved.tokens),
            runtime_gateway_sqlite_u64_to_i64(record.reserved.cost_micros),
            runtime_gateway_sqlite_u64_to_i64(actual.tokens),
            runtime_gateway_sqlite_u64_to_i64(actual.cost_micros),
            runtime_gateway_sqlite_u64_to_i64(updated),
            storage_scope,
        ],
    )?;
    if changed == 0 {
        anyhow::bail!("durable usage reconciliation was not applied");
    }
    tx.execute(
        r#"
        UPDATE prodex_reservations
        SET committed_at_unix_ms = ?8,
            released_at_unix_ms = CASE WHEN ?10 > 0 OR ?11 > 0 THEN ?8 ELSE released_at_unix_ms END
        WHERE tenant_id = ?1
          AND reservation_id = ?2
          AND call_id = ?3
          AND committed_at_unix_ms IS NULL
        "#,
        rusqlite::params![
            tenant_id,
            reservation_id,
            call_id,
            runtime_gateway_sqlite_u64_to_i64(record.reserved.tokens),
            runtime_gateway_sqlite_u64_to_i64(record.reserved.cost_micros),
            runtime_gateway_sqlite_u64_to_i64(actual.tokens),
            runtime_gateway_sqlite_u64_to_i64(actual.cost_micros),
            runtime_gateway_sqlite_u64_to_i64(updated),
            storage_scope,
            runtime_gateway_sqlite_u64_to_i64(released_tokens),
            runtime_gateway_sqlite_u64_to_i64(released_cost_micros),
        ],
    )?;
    tx.execute(
        r#"
        INSERT OR IGNORE INTO prodex_usage_ledger (
            tenant_id,
            ledger_event_id,
            reservation_id,
            call_id,
            event_kind,
            tokens,
            cost_micros,
            occurred_at_unix_ms
        ) VALUES (?1, ?12, ?2, ?3, 'committed', ?6, ?7, ?8)
        "#,
        rusqlite::params![
            tenant_id,
            reservation_id,
            call_id,
            runtime_gateway_sqlite_u64_to_i64(record.reserved.tokens),
            runtime_gateway_sqlite_u64_to_i64(record.reserved.cost_micros),
            runtime_gateway_sqlite_u64_to_i64(actual.tokens),
            runtime_gateway_sqlite_u64_to_i64(actual.cost_micros),
            runtime_gateway_sqlite_u64_to_i64(updated),
            storage_scope,
            runtime_gateway_sqlite_u64_to_i64(released_tokens),
            runtime_gateway_sqlite_u64_to_i64(released_cost_micros),
            RequestId::new().to_string(),
        ],
    )?;
    if released_tokens > 0 || released_cost_micros > 0 {
        tx.execute(
            r#"
            INSERT OR IGNORE INTO prodex_usage_ledger (
                tenant_id,
                ledger_event_id,
                reservation_id,
                call_id,
                event_kind,
                tokens,
                cost_micros,
                occurred_at_unix_ms
            ) VALUES (?1, ?12, ?2, ?3, 'released', ?10, ?11, ?8)
            "#,
            rusqlite::params![
                tenant_id,
                reservation_id,
                call_id,
                runtime_gateway_sqlite_u64_to_i64(record.reserved.tokens),
                runtime_gateway_sqlite_u64_to_i64(record.reserved.cost_micros),
                runtime_gateway_sqlite_u64_to_i64(actual.tokens),
                runtime_gateway_sqlite_u64_to_i64(actual.cost_micros),
                runtime_gateway_sqlite_u64_to_i64(updated),
                storage_scope,
                runtime_gateway_sqlite_u64_to_i64(released_tokens),
                runtime_gateway_sqlite_u64_to_i64(released_cost_micros),
                RequestId::new().to_string(),
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub(super) fn runtime_gateway_billing_ledger_reconcile_response(
    state_store: &RuntimeGatewayStateStore,
    event: &RuntimeProviderGatewaySpendEvent,
) -> std::io::Result<bool> {
    match state_store {
        RuntimeGatewayStateStore::File { ledger_path, .. } => {
            runtime_gateway_file_ledger_reconcile_response(
                ledger_path,
                event,
                runtime_gateway_unix_epoch_seconds(),
            )
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            runtime_gateway_sqlite_ledger_reconcile_response(
                path,
                event,
                runtime_gateway_unix_epoch_seconds(),
            )
            .map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Postgres { url, tls, .. } => {
            runtime_gateway_postgres_ledger_reconcile_response(
                url,
                tls,
                event,
                runtime_gateway_unix_epoch_seconds(),
            )
            .map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_redis_ledger_reconcile_response(
                url,
                RUNTIME_GATEWAY_REDIS_LEDGER_KEY,
                RUNTIME_GATEWAY_REDIS_LEDGER_LOCK,
                runtime_gateway_generate_virtual_key_token,
                event,
                runtime_gateway_unix_epoch_seconds(),
            )
            .map_err(std::io::Error::other)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_create_current_schema_for_tests;
    use super::*;
    use prodex_domain::{
        BudgetSnapshot, IdempotencyKey, ReservationRecord, ReservationRequest, TenantId,
        VirtualKeyId,
    };

    #[test]
    fn durable_actual_usage_prefers_actual_when_reservation_covers_it() {
        let tenant_id = prodex_domain::TenantId::new();
        let request = prodex_domain::ReservationRequest {
            tenant_id,
            call_id: prodex_domain::CallId::new(),
            reservation_id: prodex_domain::ReservationId::new(),
            estimate: UsageAmount::new(25, 0),
        };
        let record = prodex_domain::ReservationRecord::from_request(request, 1_000, 60_000)
            .expect("reservation record");
        let event = RuntimeProviderGatewaySpendEvent {
            event: "gateway_spend",
            phase: "response",
            request: 1,
            key_name: None,
            tenant_id: None,
            request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            legacy_request_sequence: 1,
            call_id: format!("prodex-{}", request.call_id),
            provider: "openai".to_string(),
            path: "/v1/responses".to_string(),
            model: "gpt-5.4".to_string(),
            status: 200,
            elapsed_ms: 1,
            request_bytes: 1,
            response_bytes: Some(1),
            input_tokens: Some(7),
            output_tokens: Some(11),
            cost_usd: None,
            reconciliation_reason: Some(prodex_domain::ReservationReconciliationReason::Completed),
            sink: "runtime-log".to_string(),
        };

        assert_eq!(
            runtime_gateway_durable_actual_usage(&record, &event),
            (UsageAmount::new(18, 0), false)
        );
    }

    #[test]
    fn durable_actual_usage_clamps_when_request_was_under_reserved() {
        let tenant_id = prodex_domain::TenantId::new();
        let request = prodex_domain::ReservationRequest {
            tenant_id,
            call_id: prodex_domain::CallId::new(),
            reservation_id: prodex_domain::ReservationId::new(),
            estimate: UsageAmount::new(2, 0),
        };
        let record = prodex_domain::ReservationRecord::from_request(request, 1_000, 60_000)
            .expect("reservation record");
        let event = RuntimeProviderGatewaySpendEvent {
            event: "gateway_spend",
            phase: "response",
            request: 1,
            key_name: None,
            tenant_id: None,
            request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            legacy_request_sequence: 1,
            call_id: format!("prodex-{}", request.call_id),
            provider: "openai".to_string(),
            path: "/v1/responses".to_string(),
            model: "gpt-5.4".to_string(),
            status: 200,
            elapsed_ms: 1,
            request_bytes: 1,
            response_bytes: Some(1),
            input_tokens: Some(7),
            output_tokens: Some(11),
            cost_usd: None,
            reconciliation_reason: Some(prodex_domain::ReservationReconciliationReason::Completed),
            sink: "runtime-log".to_string(),
        };

        assert_eq!(
            runtime_gateway_durable_actual_usage(&record, &event),
            (UsageAmount::new(2, 0), true)
        );
    }

    #[test]
    fn sqlite_durable_reconcile_is_idempotent_for_repeated_response_settlement() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-durable-reconcile-{}",
            prodex_domain::RequestId::new()
        ));
        std::fs::create_dir_all(&root).expect("test root should be created");
        let path = root.join("state.sqlite");
        runtime_gateway_sqlite_create_current_schema_for_tests(&path)
            .expect("sqlite schema fixture should be created");

        let tenant_id = TenantId::new();
        let virtual_key_id = VirtualKeyId::new();
        let storage_key = prodex_storage::TenantStorageKey::virtual_key(tenant_id, virtual_key_id);
        let request = ReservationRequest {
            tenant_id,
            call_id: prodex_domain::CallId::new(),
            reservation_id: prodex_domain::ReservationId::new(),
            estimate: UsageAmount::new(22, 42),
        };
        let record =
            ReservationRecord::from_request(request, 1_000, 60_000).expect("reservation record");
        let actual = UsageAmount::new(18, 29);
        let plan = prodex_storage_sqlite::plan_sqlite_usage_reconciliation(
            prodex_storage::UsageReconciliationCommand {
                storage_key,
                snapshot: BudgetSnapshot {
                    reserved: record.reserved,
                    committed: UsageAmount::ZERO,
                },
                record,
                actual,
                reason: prodex_domain::ReservationReconciliationReason::Completed,
            },
        )
        .expect("sqlite reconciliation plan");

        let conn = runtime_gateway_sqlite_open(&path).expect("sqlite database should open");
        let tenant_id_text = tenant_id.to_string();
        let virtual_key_id_text = virtual_key_id.to_string();
        let reservation_id_text = record.reservation_id.to_string();
        let call_id_text = record.call_id.to_string();
        let idempotency_key =
            IdempotencyKey::from_call_reservation(record.call_id, record.reservation_id);
        conn.execute(
            "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![tenant_id_text, "tenant", 1_i64, 1_i64],
        )
        .expect("tenant row should insert");
        conn.execute(
            "INSERT INTO prodex_budget_counters (tenant_id, storage_scope, virtual_key_id, reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros, updated_at_unix_ms) VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6)",
            rusqlite::params![
                tenant_id.to_string(),
                runtime_gateway_reconciliation_storage_scope(storage_key),
                virtual_key_id_text,
                runtime_gateway_sqlite_u64_to_i64(record.reserved.tokens),
                runtime_gateway_sqlite_u64_to_i64(record.reserved.cost_micros),
                1_000_i64,
            ],
        )
        .expect("budget counter row should insert");
        conn.execute(
            "INSERT INTO prodex_reservations (tenant_id, reservation_id, call_id, virtual_key_id, idempotency_key, reserved_tokens, reserved_cost_micros, created_at_unix_ms, expires_at_unix_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                tenant_id.to_string(),
                reservation_id_text,
                call_id_text,
                virtual_key_id.to_string(),
                idempotency_key.as_str(),
                runtime_gateway_sqlite_u64_to_i64(record.reserved.tokens),
                runtime_gateway_sqlite_u64_to_i64(record.reserved.cost_micros),
                1_000_i64,
                61_000_i64,
            ],
        )
        .expect("reservation row should insert");

        runtime_gateway_sqlite_reconcile_usage(&path, &plan, &record, actual)
            .expect("first reconcile should apply");
        runtime_gateway_sqlite_reconcile_usage(&path, &plan, &record, actual)
            .expect("second reconcile should be a no-op");

        let (reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros): (
            i64,
            i64,
            i64,
            i64,
        ) = conn
            .query_row(
                "SELECT reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros FROM prodex_budget_counters WHERE tenant_id = ?1 AND storage_scope = ?2",
                rusqlite::params![tenant_id.to_string(), runtime_gateway_reconciliation_storage_scope(storage_key)],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("budget counters should load");
        assert_eq!(reserved_tokens, 0);
        assert_eq!(reserved_cost_micros, 0);
        assert_eq!(committed_tokens, 18);
        assert_eq!(committed_cost_micros, 29);

        let committed_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM prodex_usage_ledger WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'committed'",
                rusqlite::params![tenant_id.to_string(), record.reservation_id.to_string()],
                |row| row.get(0),
            )
            .expect("committed ledger rows should load");
        let released_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM prodex_usage_ledger WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'released'",
                rusqlite::params![tenant_id.to_string(), record.reservation_id.to_string()],
                |row| row.get(0),
            )
            .expect("released ledger rows should load");
        assert_eq!(committed_rows, 1);
        assert_eq!(released_rows, 1);

        std::fs::remove_dir_all(root).expect("test root should clean up");
    }
}
