use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_application_data_plane::RuntimeGatewayApplicationAdmission;
use super::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_open;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_sqlite_utils::runtime_gateway_sqlite_u64_to_i64;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use prodex_domain::{RequestId, ReservationRecord};
use prodex_provider_core::{ProviderId, ProviderModelCost, calculate_cost_microusd};
use rusqlite::OptionalExtension;
use std::path::Path;

pub(super) enum RuntimeGatewayDurableReservationError {
    Rejected(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection),
    Failed,
}

#[derive(Clone)]
pub(super) struct RuntimeGatewayDurableReservationState {
    pub(super) storage_key: prodex_storage::TenantStorageKey,
    pub(super) record: ReservationRecord,
}

pub(super) fn runtime_gateway_limit_reservation_tokens(
    realtime: bool,
    enforcing: bool,
    has_token_or_spend_limit: bool,
    provider: ProviderId,
    model: &str,
    body: &[u8],
) -> (u64, bool) {
    let hard_limit = !realtime && enforcing && has_token_or_spend_limit;
    let tokens = if hard_limit {
        runtime_proxy_crate::runtime_gateway_hard_limit_reserved_tokens(provider, model, body)
    } else {
        runtime_proxy_crate::runtime_gateway_estimated_tokens(body)
    };
    (tokens, hard_limit)
}

pub(super) fn runtime_gateway_limit_reservation_cost(
    hard_limit: bool,
    has_spend_limit: bool,
    reserved_tokens: u64,
    input_tokens: u64,
    cost: ProviderModelCost,
) -> Result<Option<u64>, runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection> {
    let estimated = if hard_limit && has_spend_limit {
        cost.input_cost_per_million_microusd
            .zip(cost.output_cost_per_million_microusd)
            .map(|(input_rate, output_rate)| {
                reserved_tokens.saturating_mul(input_rate.max(output_rate)) / 1_000_000
            })
    } else {
        calculate_cost_microusd(
            Some(input_tokens),
            Some(reserved_tokens.saturating_sub(input_tokens)),
            cost,
        )
    };
    if hard_limit && has_spend_limit && estimated.is_none() {
        return Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable);
    }
    Ok(estimated)
}

pub(super) fn runtime_gateway_postgres_reserve_usage(
    shared: &RuntimeLocalRewriteProxyShared,
    plan: &prodex_storage_postgres::PostgresAtomicReservationSqlPlan,
    command: prodex_storage::AtomicReservationCommand,
) -> Result<(), RuntimeGatewayDurableReservationError> {
    if plan.tenant_id != command.request.tenant_id
        || plan.storage_key != command.storage_key
        || plan.idempotency_key != command.idempotency_key
    {
        return Err(RuntimeGatewayDurableReservationError::Failed);
    }
    let repository = shared
        .gateway_postgres_repository
        .as_ref()
        .ok_or(RuntimeGatewayDurableReservationError::Failed)?;
    match shared
        .runtime_shared
        .async_runtime
        .handle()
        .block_on(repository.reserve(command))
        .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?
    {
        prodex_storage_postgres_runtime::ReserveOutcome::Reserved(_)
        | prodex_storage_postgres_runtime::ReserveOutcome::Replayed(_) => Ok(()),
        prodex_storage_postgres_runtime::ReserveOutcome::Rejected(
            prodex_storage_postgres_runtime::ReserveRejection::BudgetLimitExceeded,
        ) => Err(RuntimeGatewayDurableReservationError::Rejected(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::BudgetExceeded,
        )),
        prodex_storage_postgres_runtime::ReserveOutcome::Rejected(
            prodex_storage_postgres_runtime::ReserveRejection::RequestBudgetExceeded,
        ) => Err(RuntimeGatewayDurableReservationError::Rejected(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::RequestBudgetExceeded,
        )),
        prodex_storage_postgres_runtime::ReserveOutcome::Rejected(
            prodex_storage_postgres_runtime::ReserveRejection::Conflict,
        ) => Err(RuntimeGatewayDurableReservationError::Failed),
    }
}

pub(super) fn runtime_gateway_try_durable_reservation(
    shared: &RuntimeLocalRewriteProxyShared,
    command: &prodex_storage::AtomicReservationCommand,
    application: &RuntimeGatewayApplicationAdmission,
) -> Result<Option<RuntimeGatewayDurableReservationState>, RuntimeGatewayDurableReservationError> {
    let (RuntimeGatewayStateStore::Sqlite { .. } | RuntimeGatewayStateStore::Postgres { .. }) =
        &shared.gateway_state_store
    else {
        return Ok(None);
    };
    let durable_store = match &shared.gateway_state_store {
        RuntimeGatewayStateStore::Sqlite { .. } => prodex_storage::DurableStoreKind::Sqlite,
        RuntimeGatewayStateStore::Postgres { .. } => prodex_storage::DurableStoreKind::Postgres,
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            return Ok(None);
        }
    };
    let plan = prodex_application::plan_application_atomic_reservation(
        prodex_application::ApplicationAtomicReservationRequest {
            durable_store,
            reservation: command.clone(),
        },
    )
    .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    match (&shared.gateway_state_store, plan.storage) {
        (
            RuntimeGatewayStateStore::Sqlite { path },
            prodex_application::ApplicationAtomicReservationStoragePlan::Sqlite(storage),
        ) => runtime_gateway_sqlite_reserve_usage(path, &storage, command)?,
        (
            RuntimeGatewayStateStore::Postgres { .. },
            prodex_application::ApplicationAtomicReservationStoragePlan::Postgres(storage),
        ) => runtime_gateway_postgres_reserve_usage(shared, &storage, command.clone())?,
        _ => {}
    }
    let application = application
        .tenant_bound()
        .ok_or(RuntimeGatewayDurableReservationError::Failed)?;
    let record = application.admission.reservation.reservation_record;
    if record.call_id != command.request.call_id
        || record.reservation_id != command.request.reservation_id
    {
        return Err(RuntimeGatewayDurableReservationError::Failed);
    }
    Ok(Some(RuntimeGatewayDurableReservationState {
        storage_key: command.storage_key,
        record,
    }))
}

pub(super) fn runtime_gateway_sqlite_reserve_usage(
    path: &Path,
    storage: &prodex_storage_sqlite::SqliteAtomicReservationSqlPlan,
    command: &prodex_storage::AtomicReservationCommand,
) -> Result<(), RuntimeGatewayDurableReservationError> {
    let mut conn = runtime_gateway_sqlite_open(path)
        .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    let tenant_id = storage.tenant_id.to_string();
    let idempotency_key = storage.idempotency_key.as_str().to_string();
    let existing: Option<String> = tx
        .query_row(
            "SELECT reservation_id FROM prodex_reservations WHERE tenant_id = ?1 AND idempotency_key = ?2",
            rusqlite::params![tenant_id, idempotency_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    if existing.is_some() {
        tx.commit()
            .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
        return Ok(());
    }
    let storage_scope = storage.storage_key.storage_scope();
    let virtual_key_id = storage.storage_key.virtual_key_id.map(|id| id.to_string());
    let reserved = command.request.estimate;
    let updated = runtime_gateway_unix_epoch_millis();
    let reservation_id = command.request.reservation_id.to_string();
    let call_id = command.request.call_id.to_string();
    let expires_at = updated.saturating_add(command.ttl_ms);
    let ledger_event_id = RequestId::new().to_string();
    let changed = tx
        .execute(
            r#"
            INSERT INTO prodex_budget_counters (
                tenant_id, storage_scope, virtual_key_id, reserved_tokens, reserved_cost_micros,
                committed_tokens, committed_cost_micros, updated_at_unix_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6)
            ON CONFLICT(tenant_id, storage_scope) DO UPDATE SET
                reserved_tokens = reserved_tokens + excluded.reserved_tokens,
                reserved_cost_micros = reserved_cost_micros + excluded.reserved_cost_micros,
                updated_at_unix_ms = excluded.updated_at_unix_ms
            WHERE prodex_budget_counters.tenant_id = excluded.tenant_id
              AND prodex_budget_counters.reserved_tokens + prodex_budget_counters.committed_tokens + excluded.reserved_tokens <= ?7
              AND prodex_budget_counters.reserved_cost_micros + prodex_budget_counters.committed_cost_micros + excluded.reserved_cost_micros <= ?8
            "#,
            rusqlite::params![
                tenant_id,
                storage_scope,
                virtual_key_id,
                runtime_gateway_sqlite_u64_to_i64(reserved.tokens),
                runtime_gateway_sqlite_u64_to_i64(reserved.cost_micros),
                runtime_gateway_sqlite_u64_to_i64(updated),
                runtime_gateway_sqlite_u64_to_i64(command.limit.max.tokens),
                runtime_gateway_sqlite_u64_to_i64(command.limit.max.cost_micros),
            ],
        )
        .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    if changed == 0 {
        return Err(RuntimeGatewayDurableReservationError::Rejected(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::BudgetExceeded,
        ));
    }
    tx.execute(
        r#"
        INSERT OR IGNORE INTO prodex_reservations (
            tenant_id, reservation_id, call_id, virtual_key_id, storage_scope, idempotency_key,
            reserved_tokens, reserved_cost_micros, created_at_unix_ms, expires_at_unix_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
        rusqlite::params![
            tenant_id,
            reservation_id,
            call_id,
            virtual_key_id,
            storage_scope,
            idempotency_key,
            runtime_gateway_sqlite_u64_to_i64(reserved.tokens),
            runtime_gateway_sqlite_u64_to_i64(reserved.cost_micros),
            runtime_gateway_sqlite_u64_to_i64(updated),
            runtime_gateway_sqlite_u64_to_i64(expires_at),
        ],
    )
    .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    tx.execute(
        r#"
        INSERT OR IGNORE INTO prodex_usage_ledger (
            tenant_id, ledger_event_id, reservation_id, call_id, event_kind, tokens, cost_micros, occurred_at_unix_ms
        ) VALUES (?1, ?2, ?3, ?4, 'reserved', ?5, ?6, ?7)
        "#,
        rusqlite::params![
            tenant_id,
            ledger_event_id,
            reservation_id,
            call_id,
            runtime_gateway_sqlite_u64_to_i64(reserved.tokens),
            runtime_gateway_sqlite_u64_to_i64(reserved.cost_micros),
            runtime_gateway_sqlite_u64_to_i64(updated),
        ],
    )
    .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    tx.commit()
        .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforcing_limits_reserve_ceiling_and_require_complete_pricing() {
        let body = br#"{"model":"gpt-5.4","input":"hello from prodex"}"#;
        let (tokens, hard_limit) = runtime_gateway_limit_reservation_tokens(
            false,
            true,
            true,
            ProviderId::OpenAi,
            "gpt-5.4",
            body,
        );
        assert_eq!(tokens, 400_000);
        assert!(hard_limit);
        assert_eq!(
            runtime_gateway_limit_reservation_cost(
                hard_limit,
                true,
                tokens,
                5,
                ProviderModelCost::default(),
            ),
            Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)
        );
    }
}
