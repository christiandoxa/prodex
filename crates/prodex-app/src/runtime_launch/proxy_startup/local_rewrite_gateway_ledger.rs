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
    runtime_gateway_sqlite_load_durable_reservation_state,
};
use super::local_rewrite_gateway_redis_ledger::{
    runtime_gateway_redis_ledger_load, runtime_gateway_redis_ledger_reconcile_response,
};
use super::local_rewrite_gateway_sql_ledger::{
    runtime_gateway_postgres_ledger_load, runtime_gateway_postgres_ledger_reconcile_response,
    runtime_gateway_sqlite_ledger_load, runtime_gateway_sqlite_ledger_reconcile_response,
};
#[cfg(test)]
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

#[derive(Debug)]
pub(super) enum RuntimeGatewayDurableReconciliationError {
    Conflict,
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for RuntimeGatewayDurableReconciliationError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failed(error)
    }
}

impl From<rusqlite::Error> for RuntimeGatewayDurableReconciliationError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Failed(anyhow::Error::from(error))
    }
}

impl std::fmt::Display for RuntimeGatewayDurableReconciliationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict => f.write_str(
                "durable reservation was not found or state conflicts with reconciliation",
            ),
            Self::Failed(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for RuntimeGatewayDurableReconciliationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Conflict => None,
            Self::Failed(error) => Some(error.as_ref()),
        }
    }
}

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

fn runtime_gateway_durable_reservation_state(
    durable_reservations: &Arc<Mutex<BTreeMap<u64, RuntimeGatewayDurableReservationState>>>,
    request: u64,
) -> Option<RuntimeGatewayDurableReservationState> {
    durable_reservations
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&request)
        .cloned()
}

pub(super) fn runtime_gateway_durable_reconcile_response(
    runtime_shared: &RuntimeRotationProxyShared,
    state_store: &RuntimeGatewayStateStore,
    postgres_repository: Option<&prodex_storage_postgres_runtime::PostgresRepository>,
    durable_reservations: &Arc<Mutex<BTreeMap<u64, RuntimeGatewayDurableReservationState>>>,
    event: &RuntimeProviderGatewaySpendEvent,
) -> std::io::Result<()> {
    let state = runtime_gateway_durable_reservation_state(durable_reservations, event.request);
    let state = match (state, state_store) {
        (None, RuntimeGatewayStateStore::Postgres { .. }) => {
            runtime_gateway_postgres_load_durable_reservation_state(
                runtime_shared,
                postgres_repository,
                event,
            )?
        }
        (None, RuntimeGatewayStateStore::Sqlite { path }) => {
            runtime_gateway_sqlite_load_durable_reservation_state(path, event)?
        }
        (state, _) => state,
    };
    let Some(state) = state else {
        return Ok(());
    };
    let actual = runtime_gateway_durable_actual_usage(event);
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
) -> Result<(), RuntimeGatewayDurableReconciliationError> {
    let mut conn = runtime_gateway_sqlite_open(path)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let tenant_id = plan.tenant_id.to_string();
    let reservation_id = record.reservation_id.to_string();
    let call_id = record.call_id.to_string();
    let storage_scope = runtime_gateway_reconciliation_storage_scope(plan.storage_key);
    let virtual_key_id = plan.storage_key.virtual_key_id.map(|id| id.to_string());
    let reserved_tokens = i64::try_from(record.reserved.tokens)
        .map_err(|_| RuntimeGatewayDurableReconciliationError::Conflict)?;
    let reserved_cost_micros = i64::try_from(record.reserved.cost_micros)
        .map_err(|_| RuntimeGatewayDurableReconciliationError::Conflict)?;
    let created_at = i64::try_from(record.created_at_unix_ms)
        .map_err(|_| RuntimeGatewayDurableReconciliationError::Conflict)?;
    let expires_at = i64::try_from(record.expires_at_unix_ms)
        .map_err(|_| RuntimeGatewayDurableReconciliationError::Conflict)?;
    let actual_tokens = i64::try_from(actual.tokens)
        .map_err(|_| RuntimeGatewayDurableReconciliationError::Conflict)?;
    let actual_cost_micros = i64::try_from(actual.cost_micros)
        .map_err(|_| RuntimeGatewayDurableReconciliationError::Conflict)?;
    let stored = tx
        .query_row(
            "SELECT reservation_id, call_id, virtual_key_id, storage_scope,
                    reserved_tokens, reserved_cost_micros, created_at_unix_ms, expires_at_unix_ms,
                    committed_at_unix_ms, released_at_unix_ms
             FROM prodex_reservations
             WHERE prodex_reservations.tenant_id = ?1
               AND prodex_reservations.reservation_id = ?2
               AND prodex_reservations.call_id = ?3",
            rusqlite::params![tenant_id, reservation_id, call_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                ))
            },
        )
        .optional()?;
    let Some((
        stored_reservation_id,
        stored_call_id,
        stored_virtual_key_id,
        stored_scope,
        stored_tokens,
        stored_cost_micros,
        stored_created_at,
        stored_expires_at,
        committed_at,
        released_at,
    )) = stored
    else {
        return Err(RuntimeGatewayDurableReconciliationError::Conflict);
    };
    if record.tenant_id != plan.tenant_id
        || stored_reservation_id != reservation_id
        || stored_call_id != call_id
        || stored_virtual_key_id != virtual_key_id
        || stored_scope != storage_scope
        || stored_tokens != reserved_tokens
        || stored_cost_micros != reserved_cost_micros
        || stored_created_at != created_at
        || stored_expires_at != expires_at
    {
        return Err(RuntimeGatewayDurableReconciliationError::Conflict);
    }
    let counter: Option<(Option<String>, String)> = tx
        .query_row(
            "SELECT virtual_key_id, storage_scope
             FROM prodex_budget_counters
             WHERE tenant_id = ?1 AND storage_scope = ?2",
            rusqlite::params![tenant_id, storage_scope],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if counter != Some((virtual_key_id.clone(), storage_scope.clone())) {
        return Err(RuntimeGatewayDurableReconciliationError::Conflict);
    }
    let reserved_ledger: Option<(String, String, i64, i64)> = tx
        .query_row(
            "SELECT reservation_id, call_id, tokens, cost_micros
             FROM prodex_usage_ledger
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3
               AND event_kind = 'reserved'",
            rusqlite::params![tenant_id, reservation_id, call_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    if reserved_ledger
        != Some((
            reservation_id.clone(),
            call_id.clone(),
            reserved_tokens,
            reserved_cost_micros,
        ))
    {
        return Err(RuntimeGatewayDurableReconciliationError::Conflict);
    }
    let (committed_count, committed_tokens, committed_cost_micros): (
        i64,
        Option<i64>,
        Option<i64>,
    ) = tx.query_row(
        "SELECT COUNT(*), MIN(tokens), MIN(cost_micros)
         FROM prodex_usage_ledger
         WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3
           AND event_kind = 'committed'",
        rusqlite::params![tenant_id, reservation_id, call_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let committed_usage = match (committed_count, committed_tokens, committed_cost_micros) {
        (0, None, None) => None,
        (1, Some(tokens), Some(cost_micros)) => Some((tokens, cost_micros)),
        _ => return Err(RuntimeGatewayDurableReconciliationError::Conflict),
    };
    let (released_count, released_tokens, released_cost_micros): (i64, Option<i64>, Option<i64>) =
        tx.query_row(
            "SELECT COUNT(*), MIN(tokens), MIN(cost_micros)
         FROM prodex_usage_ledger
         WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3
           AND event_kind = 'released'",
            rusqlite::params![tenant_id, reservation_id, call_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    let released_usage = match (released_count, released_tokens, released_cost_micros) {
        (0, None, None) => None,
        (1, Some(tokens), Some(cost_micros)) => Some((tokens, cost_micros)),
        _ => return Err(RuntimeGatewayDurableReconciliationError::Conflict),
    };
    let reservation_was_released = released_at.is_some();
    if (!reservation_was_released && released_usage.is_some())
        || (reservation_was_released && released_usage.is_none())
    {
        return Err(RuntimeGatewayDurableReconciliationError::Conflict);
    }
    if reservation_was_released
        && committed_at.is_none()
        && released_usage != Some((reserved_tokens, reserved_cost_micros))
    {
        return Err(RuntimeGatewayDurableReconciliationError::Conflict);
    }
    if committed_at.is_some() {
        if committed_usage != Some((actual_tokens, actual_cost_micros)) {
            return Err(RuntimeGatewayDurableReconciliationError::Conflict);
        }
        let expected_released = record.reserved.saturating_sub(actual);
        let expected_released = (
            i64::try_from(expected_released.tokens)
                .map_err(|_| RuntimeGatewayDurableReconciliationError::Conflict)?,
            i64::try_from(expected_released.cost_micros)
                .map_err(|_| RuntimeGatewayDurableReconciliationError::Conflict)?,
        );
        if (reservation_was_released
            && released_usage != Some((reserved_tokens, reserved_cost_micros))
            && released_usage != Some(expected_released))
            || (!reservation_was_released && expected_released != (0, 0))
        {
            return Err(RuntimeGatewayDurableReconciliationError::Conflict);
        }
        tx.commit()?;
        return Ok(());
    }
    if committed_usage.is_some() {
        return Err(RuntimeGatewayDurableReconciliationError::Conflict);
    }
    let updated = i64::try_from(runtime_gateway_unix_epoch_millis())
        .map_err(|_| RuntimeGatewayDurableReconciliationError::Conflict)?;
    let reserved_tokens_to_release = if reservation_was_released {
        0
    } else {
        reserved_tokens
    };
    let reserved_cost_to_release = if reservation_was_released {
        0
    } else {
        reserved_cost_micros
    };
    let released = if reservation_was_released {
        UsageAmount::ZERO
    } else {
        record.reserved.saturating_sub(actual)
    };
    let released_tokens = released.tokens;
    let released_cost_micros = released.cost_micros;
    let released_tokens_i64 = i64::try_from(released_tokens)
        .map_err(|_| RuntimeGatewayDurableReconciliationError::Conflict)?;
    let released_cost_micros_i64 = i64::try_from(released_cost_micros)
        .map_err(|_| RuntimeGatewayDurableReconciliationError::Conflict)?;
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
            reserved_tokens_to_release,
            reserved_cost_to_release,
            actual_tokens,
            actual_cost_micros,
            updated,
            storage_scope,
        ],
    )?;
    if changed == 0 {
        return Err(RuntimeGatewayDurableReconciliationError::Conflict);
    }
    let reservation_updated = tx.execute(
        r#"
        UPDATE prodex_reservations
        SET committed_at_unix_ms = ?8,
            released_at_unix_ms = CASE
                WHEN released_at_unix_ms IS NOT NULL THEN released_at_unix_ms
                WHEN ?10 > 0 OR ?11 > 0 THEN ?8
                ELSE NULL
            END
        WHERE tenant_id = ?1
          AND reservation_id = ?2
          AND call_id = ?3
          AND committed_at_unix_ms IS NULL
        "#,
        rusqlite::params![
            tenant_id,
            reservation_id,
            call_id,
            reserved_tokens,
            reserved_cost_micros,
            actual_tokens,
            actual_cost_micros,
            updated,
            storage_scope,
            released_tokens_i64,
            released_cost_micros_i64,
        ],
    )?;
    if reservation_updated != 1 {
        return Err(RuntimeGatewayDurableReconciliationError::Conflict);
    }
    tx.execute(
        r#"
        INSERT INTO prodex_usage_ledger (
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
            reserved_tokens,
            reserved_cost_micros,
            actual_tokens,
            actual_cost_micros,
            updated,
            storage_scope,
            released_tokens_i64,
            released_cost_micros_i64,
            RequestId::new().to_string(),
        ],
    )?;
    if released_tokens > 0 || released_cost_micros > 0 {
        tx.execute(
            r#"
            INSERT INTO prodex_usage_ledger (
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
                reserved_tokens,
                reserved_cost_micros,
                actual_tokens,
                actual_cost_micros,
                updated,
                storage_scope,
                released_tokens_i64,
                released_cost_micros_i64,
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
        BudgetSnapshot, CallId, IdempotencyKey, ReservationRecord, ReservationRequest, TenantId,
        VirtualKeyId,
    };
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    type SqliteReconciliationState = ((i64, i64, i64, i64), (i64, i64, i64), i64);

    fn sqlite_reconciliation_state(
        path: &Path,
        tenant_id: TenantId,
        storage_key: prodex_storage::TenantStorageKey,
        reservation_id: prodex_domain::ReservationId,
    ) -> SqliteReconciliationState {
        let conn = runtime_gateway_sqlite_open(path).expect("sqlite database should open");
        let counters = conn
            .query_row(
                "SELECT reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros FROM prodex_budget_counters WHERE tenant_id = ?1 AND storage_scope = ?2",
                rusqlite::params![tenant_id.to_string(), runtime_gateway_reconciliation_storage_scope(storage_key)],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("budget counters should load");
        let committed = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(tokens), 0), COALESCE(SUM(cost_micros), 0) FROM prodex_usage_ledger WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'committed'",
                rusqlite::params![tenant_id.to_string(), reservation_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("committed ledger should load");
        let released = conn
            .query_row(
                "SELECT COUNT(*) FROM prodex_usage_ledger WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'released'",
                rusqlite::params![tenant_id.to_string(), reservation_id.to_string()],
                |row| row.get(0),
            )
            .expect("released ledger should load");
        (counters, committed, released)
    }

    #[allow(clippy::too_many_arguments)]
    fn assert_sqlite_reconciliation_conflict_unchanged(
        path: &Path,
        plan: &prodex_storage_sqlite::SqliteUsageReconciliationSqlPlan,
        record: &ReservationRecord,
        actual: UsageAmount,
        expected: SqliteReconciliationState,
        tenant_id: TenantId,
        storage_key: prodex_storage::TenantStorageKey,
        reservation_id: prodex_domain::ReservationId,
        label: &str,
    ) {
        let error = runtime_gateway_sqlite_reconcile_usage(path, plan, record, actual)
            .expect_err("reconciliation mismatch must fail");
        assert!(
            matches!(&error, &RuntimeGatewayDurableReconciliationError::Conflict),
            "{label} mismatch should be a typed conflict: {error}"
        );
        assert_eq!(
            sqlite_reconciliation_state(path, tenant_id, storage_key, reservation_id),
            expected,
            "{label} mismatch must not mutate counters or ledger"
        );
    }

    #[test]
    fn durable_reservation_state_lookup_recovers_poisoned_mutex() {
        let tenant_id = TenantId::new();
        let record = ReservationRecord::from_request(
            ReservationRequest {
                tenant_id,
                call_id: prodex_domain::CallId::new(),
                reservation_id: prodex_domain::ReservationId::new(),
                estimate: UsageAmount::new(1, 1),
            },
            1,
            60_000,
        )
        .expect("reservation record");
        let request = 41;
        let reservations = Arc::new(Mutex::new(BTreeMap::from([(
            request,
            RuntimeGatewayDurableReservationState {
                storage_key: prodex_storage::TenantStorageKey::tenant(tenant_id),
                record,
            },
        )])));
        let poisoned = Arc::clone(&reservations);
        assert!(
            std::thread::spawn(move || {
                let _guard = poisoned.lock().expect("reservation state lock");
                panic!("poison reservation state lock");
            })
            .join()
            .is_err()
        );

        let recovered = runtime_gateway_durable_reservation_state(&reservations, request)
            .expect("poisoned reservation state should be recovered");
        assert_eq!(recovered.record.reservation_id, record.reservation_id);
    }

    #[test]
    fn sqlite_policy_key_without_durable_reservation_is_not_retried() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-policy-reconcile-{}",
            prodex_domain::RequestId::new()
        ));
        std::fs::create_dir_all(&root).expect("test root should be created");
        let path = root.join("state.sqlite");
        runtime_gateway_sqlite_create_current_schema_for_tests(&path)
            .expect("sqlite schema fixture should be created");
        let event = RuntimeProviderGatewaySpendEvent {
            event: "gateway_spend",
            phase: "response",
            request: 7,
            key_name: Some("policy-key".to_string()),
            tenant_id: Some(TenantId::new().to_string()),
            request_id: format!("prodex-{}", prodex_domain::RequestId::new()),
            legacy_request_sequence: 7,
            call_id: format!("prodex-{}", prodex_domain::CallId::new()),
            provider: "openai".to_string(),
            path: "/v1/responses".to_string(),
            model: "gpt-5.4".to_string(),
            status: 200,
            elapsed_ms: 1,
            request_bytes: 1,
            response_bytes: Some(1),
            input_tokens: Some(1),
            output_tokens: Some(1),
            cost_usd: None,
            reconciliation_reason: Some(prodex_domain::ReservationReconciliationReason::Completed),
            sink: "runtime-log".to_string(),
        };
        let state = runtime_gateway_sqlite_load_durable_reservation_state(&path, &event)
            .expect("policy-key lookup should succeed");
        assert!(state.is_none());
        std::fs::remove_dir_all(root).expect("test root should clean up");
    }

    #[test]
    fn durable_actual_usage_prefers_actual_when_reservation_covers_it() {
        let tenant_id = prodex_domain::TenantId::new();
        let request = prodex_domain::ReservationRequest {
            tenant_id,
            call_id: prodex_domain::CallId::new(),
            reservation_id: prodex_domain::ReservationId::new(),
            estimate: UsageAmount::new(25, 0),
        };
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
            runtime_gateway_durable_actual_usage(&event),
            UsageAmount::new(18, 0)
        );
    }

    #[test]
    fn durable_actual_usage_records_overage_when_request_was_under_reserved() {
        let tenant_id = prodex_domain::TenantId::new();
        let request = prodex_domain::ReservationRequest {
            tenant_id,
            call_id: prodex_domain::CallId::new(),
            reservation_id: prodex_domain::ReservationId::new(),
            estimate: UsageAmount::new(2, 0),
        };
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
            runtime_gateway_durable_actual_usage(&event),
            UsageAmount::new(18, 0)
        );
    }

    #[test]
    fn sqlite_durable_reconcile_missing_reservation_is_error() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-missing-reservation-{}",
            prodex_domain::RequestId::new()
        ));
        std::fs::create_dir_all(&root).expect("test root should be created");
        let path = root.join("state.sqlite");
        runtime_gateway_sqlite_create_current_schema_for_tests(&path)
            .expect("sqlite schema fixture should be created");

        let tenant_id = TenantId::new();
        let storage_key = prodex_storage::TenantStorageKey::tenant(tenant_id);
        let record = ReservationRecord::from_request(
            ReservationRequest {
                tenant_id,
                call_id: prodex_domain::CallId::new(),
                reservation_id: prodex_domain::ReservationId::new(),
                estimate: UsageAmount::new(2, 3),
            },
            1_000,
            60_000,
        )
        .expect("reservation record");
        let actual = UsageAmount::new(1, 1);
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

        let error = runtime_gateway_sqlite_reconcile_usage(&path, &plan, &record, actual)
            .expect_err("missing durable reservation must not reconcile successfully");
        assert!(matches!(
            &error,
            &RuntimeGatewayDurableReconciliationError::Conflict
        ));
        assert!(
            error
                .to_string()
                .contains("durable reservation was not found")
        );

        std::fs::remove_dir_all(root).expect("test root should clean up");
    }

    #[test]
    fn sqlite_durable_reconcile_records_overage_idempotently() {
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
        let actual = UsageAmount::new(28, 49);
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
            "INSERT INTO prodex_reservations (tenant_id, reservation_id, call_id, virtual_key_id, storage_scope, idempotency_key, reserved_tokens, reserved_cost_micros, created_at_unix_ms, expires_at_unix_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                tenant_id.to_string(),
                reservation_id_text,
                call_id_text,
                virtual_key_id.to_string(),
                runtime_gateway_reconciliation_storage_scope(storage_key),
                idempotency_key.as_str(),
                runtime_gateway_sqlite_u64_to_i64(record.reserved.tokens),
                runtime_gateway_sqlite_u64_to_i64(record.reserved.cost_micros),
                1_000_i64,
                61_000_i64,
            ],
        )
        .expect("reservation row should insert");
        conn.execute(
            "INSERT INTO prodex_usage_ledger
             (tenant_id, ledger_event_id, reservation_id, call_id, event_kind,
              tokens, cost_micros, occurred_at_unix_ms)
             VALUES (?1, ?2, ?3, ?4, 'reserved', ?5, ?6, ?7)",
            rusqlite::params![
                tenant_id.to_string(),
                RequestId::new().to_string(),
                record.reservation_id.to_string(),
                record.call_id.to_string(),
                runtime_gateway_sqlite_u64_to_i64(record.reserved.tokens),
                runtime_gateway_sqlite_u64_to_i64(record.reserved.cost_micros),
                record.created_at_unix_ms as i64,
            ],
        )
        .expect("reserved ledger row should insert");

        runtime_gateway_sqlite_reconcile_usage(&path, &plan, &record, actual)
            .expect("first reconcile should apply");
        let applied_state =
            sqlite_reconciliation_state(&path, tenant_id, storage_key, record.reservation_id);
        runtime_gateway_sqlite_reconcile_usage(&path, &plan, &record, actual)
            .expect("second reconcile should be a no-op");
        assert_eq!(
            sqlite_reconciliation_state(&path, tenant_id, storage_key, record.reservation_id),
            applied_state
        );
        let replay_state = applied_state;
        let mut virtual_key_mismatch = storage_key;
        virtual_key_mismatch.virtual_key_id = Some(VirtualKeyId::new());
        let virtual_key_plan = prodex_storage_sqlite::plan_sqlite_usage_reconciliation(
            prodex_storage::UsageReconciliationCommand {
                storage_key: virtual_key_mismatch,
                snapshot: BudgetSnapshot {
                    reserved: record.reserved,
                    committed: UsageAmount::ZERO,
                },
                record,
                actual,
                reason: prodex_domain::ReservationReconciliationReason::Completed,
            },
        )
        .expect("virtual key mismatch plan should be valid");
        assert_sqlite_reconciliation_conflict_unchanged(
            &path,
            &virtual_key_plan,
            &record,
            actual,
            replay_state,
            tenant_id,
            storage_key,
            record.reservation_id,
            "virtual key",
        );

        let scope_mismatch = prodex_storage::TenantStorageKey::budget_group(
            tenant_id,
            storage_key.virtual_key_id.expect("virtual key"),
            prodex_storage::BudgetStorageScope::from_digest([8; 32]),
        );
        let scope_plan = prodex_storage_sqlite::plan_sqlite_usage_reconciliation(
            prodex_storage::UsageReconciliationCommand {
                storage_key: scope_mismatch,
                snapshot: BudgetSnapshot {
                    reserved: record.reserved,
                    committed: UsageAmount::ZERO,
                },
                record,
                actual,
                reason: prodex_domain::ReservationReconciliationReason::Completed,
            },
        )
        .expect("storage scope mismatch plan should be valid");
        assert_sqlite_reconciliation_conflict_unchanged(
            &path,
            &scope_plan,
            &record,
            actual,
            replay_state,
            tenant_id,
            storage_key,
            record.reservation_id,
            "storage scope",
        );

        let mut tenant_record = record;
        tenant_record.tenant_id = TenantId::new();
        assert_sqlite_reconciliation_conflict_unchanged(
            &path,
            &plan,
            &tenant_record,
            actual,
            replay_state,
            tenant_id,
            storage_key,
            record.reservation_id,
            "tenant",
        );

        let mut call_record = record;
        call_record.call_id = CallId::new();
        assert_sqlite_reconciliation_conflict_unchanged(
            &path,
            &plan,
            &call_record,
            actual,
            replay_state,
            tenant_id,
            storage_key,
            record.reservation_id,
            "call",
        );

        let mut reservation_record = record;
        reservation_record.reservation_id = prodex_domain::ReservationId::new();
        assert_sqlite_reconciliation_conflict_unchanged(
            &path,
            &plan,
            &reservation_record,
            actual,
            replay_state,
            tenant_id,
            storage_key,
            record.reservation_id,
            "reservation",
        );

        let mut amount_record = record;
        amount_record.reserved = UsageAmount::new(21, 41);
        assert_sqlite_reconciliation_conflict_unchanged(
            &path,
            &plan,
            &amount_record,
            actual,
            replay_state,
            tenant_id,
            storage_key,
            record.reservation_id,
            "reserved amount",
        );

        let mut created_at_record = record;
        created_at_record.created_at_unix_ms += 1;
        assert_sqlite_reconciliation_conflict_unchanged(
            &path,
            &plan,
            &created_at_record,
            actual,
            replay_state,
            tenant_id,
            storage_key,
            record.reservation_id,
            "created at",
        );

        let mut expires_at_record = record;
        expires_at_record.expires_at_unix_ms += 1;
        assert_sqlite_reconciliation_conflict_unchanged(
            &path,
            &plan,
            &expires_at_record,
            actual,
            replay_state,
            tenant_id,
            storage_key,
            record.reservation_id,
            "expires at",
        );

        let counters_before_conflict: (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros FROM prodex_budget_counters WHERE tenant_id = ?1 AND storage_scope = ?2",
                rusqlite::params![tenant_id.to_string(), runtime_gateway_reconciliation_storage_scope(storage_key)],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("budget counters should load before conflict");
        let ledger_before_conflict: (i64, i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), tokens, cost_micros FROM prodex_usage_ledger WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'committed'",
                rusqlite::params![tenant_id.to_string(), record.reservation_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("committed ledger should load before conflict");
        for (label, mismatch_actual) in [
            ("actual tokens", UsageAmount::new(27, 49)),
            ("actual cost", UsageAmount::new(28, 48)),
        ] {
            let mismatch_plan = prodex_storage_sqlite::plan_sqlite_usage_reconciliation(
                prodex_storage::UsageReconciliationCommand {
                    storage_key,
                    snapshot: BudgetSnapshot {
                        reserved: record.reserved,
                        committed: UsageAmount::ZERO,
                    },
                    record,
                    actual: mismatch_actual,
                    reason: prodex_domain::ReservationReconciliationReason::Completed,
                },
            )
            .expect("mismatched replay plan should be valid");
            let error = runtime_gateway_sqlite_reconcile_usage(
                &path,
                &mismatch_plan,
                &record,
                mismatch_actual,
            )
            .expect_err("different committed usage must be a typed conflict");
            assert!(
                matches!(&error, &RuntimeGatewayDurableReconciliationError::Conflict),
                "{label} mismatch should be a typed conflict: {error}"
            );
        }
        let counters_after_conflict: (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros FROM prodex_budget_counters WHERE tenant_id = ?1 AND storage_scope = ?2",
                rusqlite::params![tenant_id.to_string(), runtime_gateway_reconciliation_storage_scope(storage_key)],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("budget counters should load after conflict");
        let ledger_after_conflict: (i64, i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), tokens, cost_micros FROM prodex_usage_ledger WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'committed'",
                rusqlite::params![tenant_id.to_string(), record.reservation_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("committed ledger should load after conflict");
        assert_eq!(counters_after_conflict, counters_before_conflict);
        assert_eq!(ledger_after_conflict, ledger_before_conflict);

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
        assert_eq!(committed_tokens, 28);
        assert_eq!(committed_cost_micros, 49);

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
        assert_eq!(released_rows, 0);

        conn.execute(
            "UPDATE prodex_usage_ledger SET call_id = ?1
             WHERE tenant_id = ?2 AND reservation_id = ?3 AND event_kind = 'committed'",
            rusqlite::params![
                CallId::new().to_string(),
                tenant_id.to_string(),
                record.reservation_id.to_string(),
            ],
        )
        .expect("test ledger tamper should apply");
        let tampered_state =
            sqlite_reconciliation_state(&path, tenant_id, storage_key, record.reservation_id);
        let error = runtime_gateway_sqlite_reconcile_usage(&path, &plan, &record, actual)
            .expect_err("committed ledger call mismatch must conflict");
        assert!(matches!(
            error,
            RuntimeGatewayDurableReconciliationError::Conflict
        ));
        assert_eq!(
            sqlite_reconciliation_state(&path, tenant_id, storage_key, record.reservation_id),
            tampered_state
        );

        drop(conn);
        std::fs::remove_dir_all(root).expect("test root should clean up");
    }

    #[test]
    fn sqlite_late_reconcile_preserves_unrelated_reservation() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-late-reconcile-{}",
            prodex_domain::RequestId::new()
        ));
        std::fs::create_dir_all(&root).expect("test root should be created");
        let path = root.join("state.sqlite");
        runtime_gateway_sqlite_create_current_schema_for_tests(&path)
            .expect("sqlite schema fixture should be created");

        let tenant_id = TenantId::new();
        let virtual_key_id = VirtualKeyId::new();
        let storage_key = prodex_storage::TenantStorageKey::virtual_key(tenant_id, virtual_key_id);
        let expired = ReservationRecord::from_request(
            ReservationRequest {
                tenant_id,
                call_id: prodex_domain::CallId::new(),
                reservation_id: prodex_domain::ReservationId::new(),
                estimate: UsageAmount::new(22, 42),
            },
            100,
            100,
        )
        .expect("expired reservation record");
        let active = ReservationRecord::from_request(
            ReservationRequest {
                tenant_id,
                call_id: prodex_domain::CallId::new(),
                reservation_id: prodex_domain::ReservationId::new(),
                estimate: UsageAmount::new(13, 17),
            },
            200,
            10_000,
        )
        .expect("active reservation record");
        let conn = runtime_gateway_sqlite_open(&path).expect("sqlite database should open");
        conn.execute(
            "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms) VALUES (?1, 'tenant', 1, 1)",
            rusqlite::params![tenant_id.to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO prodex_budget_counters (tenant_id, storage_scope, virtual_key_id, reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros, updated_at_unix_ms) VALUES (?1, ?2, ?3, 35, 59, 0, 0, 200)",
            rusqlite::params![
                tenant_id.to_string(),
                runtime_gateway_reconciliation_storage_scope(storage_key),
                virtual_key_id.to_string(),
            ],
        )
        .unwrap();
        for record in [expired, active] {
            let idempotency_key =
                IdempotencyKey::from_call_reservation(record.call_id, record.reservation_id);
            conn.execute(
                "INSERT INTO prodex_reservations (tenant_id, reservation_id, call_id, virtual_key_id, storage_scope, idempotency_key, reserved_tokens, reserved_cost_micros, created_at_unix_ms, expires_at_unix_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    tenant_id.to_string(),
                    record.reservation_id.to_string(),
                    record.call_id.to_string(),
                    virtual_key_id.to_string(),
                    runtime_gateway_reconciliation_storage_scope(storage_key),
                    idempotency_key.as_str(),
                    runtime_gateway_sqlite_u64_to_i64(record.reserved.tokens),
                    runtime_gateway_sqlite_u64_to_i64(record.reserved.cost_micros),
                    runtime_gateway_sqlite_u64_to_i64(record.created_at_unix_ms),
                    runtime_gateway_sqlite_u64_to_i64(record.expires_at_unix_ms),
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO prodex_usage_ledger
                 (tenant_id, ledger_event_id, reservation_id, call_id, event_kind,
                  tokens, cost_micros, occurred_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, 'reserved', ?5, ?6, ?7)",
                rusqlite::params![
                    tenant_id.to_string(),
                    RequestId::new().to_string(),
                    record.reservation_id.to_string(),
                    record.call_id.to_string(),
                    runtime_gateway_sqlite_u64_to_i64(record.reserved.tokens),
                    runtime_gateway_sqlite_u64_to_i64(record.reserved.cost_micros),
                    record.created_at_unix_ms as i64,
                ],
            )
            .unwrap();
        }
        drop(conn);

        let mut repository =
            prodex_storage_sqlite_runtime::SqliteAccountingRepository::open(&path).unwrap();
        repository
            .release_expired(prodex_storage::ExpiredReservationRecoveryCommand {
                storage_key,
                snapshot: BudgetSnapshot {
                    reserved: UsageAmount::new(35, 59),
                    committed: UsageAmount::ZERO,
                },
                record: expired,
                now_unix_ms: 300,
            })
            .expect("expired reservation should release");
        drop(repository);
        let conn = runtime_gateway_sqlite_open(&path).unwrap();
        let counters_after_recovery: (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros FROM prodex_budget_counters WHERE tenant_id = ?1 AND storage_scope = ?2",
                rusqlite::params![tenant_id.to_string(), runtime_gateway_reconciliation_storage_scope(storage_key)],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
        assert_eq!(counters_after_recovery, (13, 17, 0, 0));
        drop(conn);

        let actual = UsageAmount::new(9, 11);
        let plan = prodex_storage_sqlite::plan_sqlite_usage_reconciliation(
            prodex_storage::UsageReconciliationCommand {
                storage_key,
                snapshot: BudgetSnapshot {
                    reserved: expired.reserved,
                    committed: UsageAmount::ZERO,
                },
                record: expired,
                actual,
                reason: prodex_domain::ReservationReconciliationReason::Completed,
            },
        )
        .unwrap();
        let conn = runtime_gateway_sqlite_open(&path).unwrap();
        conn.execute(
            "UPDATE prodex_usage_ledger SET cost_micros = 41
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'released'",
            rusqlite::params![tenant_id.to_string(), expired.reservation_id.to_string()],
        )
        .expect("test ledger tamper should apply");
        let tampered_state =
            sqlite_reconciliation_state(&path, tenant_id, storage_key, expired.reservation_id);
        let error = runtime_gateway_sqlite_reconcile_usage(&path, &plan, &expired, actual)
            .expect_err("partial pre-commit release must conflict");
        assert!(matches!(
            error,
            RuntimeGatewayDurableReconciliationError::Conflict
        ));
        assert_eq!(
            sqlite_reconciliation_state(&path, tenant_id, storage_key, expired.reservation_id),
            tampered_state
        );
        conn.execute(
            "UPDATE prodex_usage_ledger SET cost_micros = 42
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'released'",
            rusqlite::params![tenant_id.to_string(), expired.reservation_id.to_string()],
        )
        .expect("test ledger repair should apply");
        drop(conn);
        runtime_gateway_sqlite_reconcile_usage(&path, &plan, &expired, actual)
            .expect("late reconciliation should apply");

        let conn = runtime_gateway_sqlite_open(&path).unwrap();
        conn.execute(
            "UPDATE prodex_usage_ledger SET cost_micros = 41
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'released'",
            rusqlite::params![tenant_id.to_string(), expired.reservation_id.to_string()],
        )
        .expect("test ledger tamper should apply");
        let tampered_state =
            sqlite_reconciliation_state(&path, tenant_id, storage_key, expired.reservation_id);
        let error = runtime_gateway_sqlite_reconcile_usage(&path, &plan, &expired, actual)
            .expect_err("released ledger amount mismatch must conflict");
        assert!(matches!(
            error,
            RuntimeGatewayDurableReconciliationError::Conflict
        ));
        assert_eq!(
            sqlite_reconciliation_state(&path, tenant_id, storage_key, expired.reservation_id),
            tampered_state
        );
        conn.execute(
            "UPDATE prodex_usage_ledger SET cost_micros = 42
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'released'",
            rusqlite::params![tenant_id.to_string(), expired.reservation_id.to_string()],
        )
        .expect("test ledger repair should apply");
        drop(conn);
        runtime_gateway_sqlite_reconcile_usage(&path, &plan, &expired, actual)
            .expect("late reconciliation replay should be idempotent");

        let conn = runtime_gateway_sqlite_open(&path).unwrap();
        conn.execute(
            "UPDATE prodex_reservations SET released_at_unix_ms = NULL
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3",
            rusqlite::params![
                tenant_id.to_string(),
                expired.reservation_id.to_string(),
                expired.call_id.to_string(),
            ],
        )
        .expect("test release marker tamper should apply");
        conn.execute(
            "DELETE FROM prodex_usage_ledger
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3
               AND event_kind = 'released'",
            rusqlite::params![
                tenant_id.to_string(),
                expired.reservation_id.to_string(),
                expired.call_id.to_string(),
            ],
        )
        .expect("test release ledger tamper should apply");
        let tampered_state =
            sqlite_reconciliation_state(&path, tenant_id, storage_key, expired.reservation_id);
        let error = runtime_gateway_sqlite_reconcile_usage(&path, &plan, &expired, actual)
            .expect_err("missing release state must conflict");
        assert!(matches!(
            error,
            RuntimeGatewayDurableReconciliationError::Conflict
        ));
        assert_eq!(
            sqlite_reconciliation_state(&path, tenant_id, storage_key, expired.reservation_id),
            tampered_state
        );
        conn.execute(
            "UPDATE prodex_reservations SET released_at_unix_ms = 300
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3",
            rusqlite::params![
                tenant_id.to_string(),
                expired.reservation_id.to_string(),
                expired.call_id.to_string(),
            ],
        )
        .expect("test release marker repair should apply");
        conn.execute(
            "INSERT INTO prodex_usage_ledger
             (tenant_id, ledger_event_id, reservation_id, call_id, event_kind,
              tokens, cost_micros, occurred_at_unix_ms)
             VALUES (?1, ?2, ?3, ?4, 'released', ?5, ?6, 300)",
            rusqlite::params![
                tenant_id.to_string(),
                RequestId::new().to_string(),
                expired.reservation_id.to_string(),
                expired.call_id.to_string(),
                runtime_gateway_sqlite_u64_to_i64(expired.reserved.tokens),
                runtime_gateway_sqlite_u64_to_i64(expired.reserved.cost_micros),
            ],
        )
        .expect("test release ledger repair should apply");
        drop(conn);

        let conn = runtime_gateway_sqlite_open(&path).expect("sqlite database should reopen");
        let counters: (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros FROM prodex_budget_counters WHERE tenant_id = ?1 AND storage_scope = ?2",
                rusqlite::params![tenant_id.to_string(), runtime_gateway_reconciliation_storage_scope(storage_key)],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(counters, (13, 17, 9, 11));
        let ledger: (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*) FILTER (WHERE event_kind = 'released'), COUNT(*) FILTER (WHERE event_kind = 'committed') FROM prodex_usage_ledger WHERE tenant_id = ?1 AND reservation_id = ?2",
                rusqlite::params![tenant_id.to_string(), expired.reservation_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(ledger, (1, 1));
        drop(conn);

        std::fs::remove_dir_all(root).expect("test root should clean up");
    }
}
