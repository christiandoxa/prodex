use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_application_data_plane::RuntimeGatewayApplicationAdmission;
use super::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_open;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_sqlite_utils::runtime_gateway_sqlite_u64_to_i64;
use prodex_domain::{RequestId, ReservationRecord};
use prodex_provider_core::{ProviderId, ProviderModelCost, calculate_cost_microusd};
use rusqlite::OptionalExtension;
use std::path::Path;

type ExistingReservationReplay = (
    String,
    String,
    Option<String>,
    String,
    String,
    i64,
    i64,
    i64,
    i64,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeGatewayDurableReservationError {
    Rejected(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection),
    Conflict,
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
        return Err(RuntimeGatewayDurableReservationError::Conflict);
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
        ) => Err(RuntimeGatewayDurableReservationError::Conflict),
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
    if storage.tenant_id != command.request.tenant_id
        || storage.storage_key != command.storage_key
        || storage.idempotency_key != command.idempotency_key
    {
        return Err(RuntimeGatewayDurableReservationError::Conflict);
    }
    let expires_at = command
        .created_at_unix_ms
        .checked_add(command.ttl_ms)
        .ok_or(RuntimeGatewayDurableReservationError::Failed)?;
    let Some(values) = [
        command.request.estimate.tokens,
        command.request.estimate.cost_micros,
        command.created_at_unix_ms,
        expires_at,
    ]
    .into_iter()
    .map(|value| i64::try_from(value).ok())
    .collect::<Option<Vec<_>>>() else {
        return Err(RuntimeGatewayDurableReservationError::Failed);
    };
    let &[
        reserved_tokens,
        reserved_cost_micros,
        created_at,
        expires_at,
    ] = values.as_slice()
    else {
        return Err(RuntimeGatewayDurableReservationError::Failed);
    };
    let max_tokens = runtime_gateway_sqlite_u64_to_i64(command.limit.max.tokens);
    let max_cost = runtime_gateway_sqlite_u64_to_i64(command.limit.max.cost_micros);
    let mut conn = runtime_gateway_sqlite_open(path)
        .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    let tenant_id = storage.tenant_id.to_string();
    let idempotency_key = storage.idempotency_key.as_str().to_string();
    let storage_scope = storage.storage_key.storage_scope();
    let virtual_key_id = storage.storage_key.virtual_key_id.map(|id| id.to_string());
    let reservation_id = command.request.reservation_id.to_string();
    let call_id = command.request.call_id.to_string();
    let existing: Option<ExistingReservationReplay> = {
        let mut statement = tx
            .prepare(
                "SELECT reservation_id, call_id, virtual_key_id, storage_scope, idempotency_key,
                        reserved_tokens, reserved_cost_micros, created_at_unix_ms, expires_at_unix_ms
                 FROM prodex_reservations
                 WHERE tenant_id = ?1
                   AND (reservation_id = ?2 OR call_id = ?3 OR idempotency_key = ?4)",
            )
            .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
        let rows = statement
            .query_map(
                rusqlite::params![&tenant_id, &reservation_id, &call_id, &idempotency_key],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
        if rows.len() > 1 {
            return Err(RuntimeGatewayDurableReservationError::Conflict);
        }
        rows.into_iter().next()
    };
    if let Some((
        stored_reservation_id,
        stored_call_id,
        stored_virtual_key_id,
        stored_scope,
        stored_idempotency_key,
        stored_tokens,
        stored_cost_micros,
        stored_created_at,
        stored_expires_at,
    )) = existing
    {
        let counter: Option<(Option<String>, String)> = tx
            .query_row(
                "SELECT virtual_key_id, storage_scope
                 FROM prodex_budget_counters
                 WHERE tenant_id = ?1 AND storage_scope = ?2",
                rusqlite::params![tenant_id, storage_scope],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
        let reserved_ledger: Option<(String, String, i64, i64)> = tx
            .query_row(
                "SELECT reservation_id, call_id, tokens, cost_micros
                 FROM prodex_usage_ledger
                 WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3
                   AND event_kind = 'reserved'",
                rusqlite::params![tenant_id, reservation_id, call_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
        let exact = stored_reservation_id == reservation_id
            && stored_call_id == call_id
            && stored_virtual_key_id == virtual_key_id
            && stored_scope == storage_scope
            && stored_idempotency_key == idempotency_key
            && stored_tokens == reserved_tokens
            && stored_cost_micros == reserved_cost_micros
            && stored_created_at == created_at
            && stored_expires_at == expires_at
            && counter == Some((virtual_key_id.clone(), storage_scope.clone()))
            && reserved_ledger
                == Some((
                    reservation_id.clone(),
                    call_id.clone(),
                    reserved_tokens,
                    reserved_cost_micros,
                ));
        if exact {
            tx.commit()
                .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
            return Ok(());
        }
        return Err(RuntimeGatewayDurableReservationError::Conflict);
    }
    let counter: Option<(Option<String>, String)> = tx
        .query_row(
            "SELECT virtual_key_id, storage_scope
             FROM prodex_budget_counters
             WHERE tenant_id = ?1 AND storage_scope = ?2",
            rusqlite::params![&tenant_id, &storage_scope],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    if counter.is_some_and(|value| value != (virtual_key_id.clone(), storage_scope.clone())) {
        return Err(RuntimeGatewayDurableReservationError::Conflict);
    }
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
                reserved_tokens,
                reserved_cost_micros,
                created_at,
                max_tokens,
                max_cost,
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
        INSERT INTO prodex_reservations (
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
            reserved_tokens,
            reserved_cost_micros,
            created_at,
            expires_at,
        ],
    )
    .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    tx.execute(
        r#"
        INSERT INTO prodex_usage_ledger (
            tenant_id, ledger_event_id, reservation_id, call_id, event_kind, tokens, cost_micros, occurred_at_unix_ms
        ) VALUES (?1, ?2, ?3, ?4, 'reserved', ?5, ?6, ?7)
        "#,
        rusqlite::params![
            tenant_id,
            ledger_event_id,
            reservation_id,
            call_id,
            reserved_tokens,
            reserved_cost_micros,
            created_at,
        ],
    )
    .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    tx.commit()
        .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::local_rewrite_gateway_backend_connection::{
        runtime_gateway_sqlite_create_current_schema_for_tests, runtime_gateway_sqlite_open,
    };
    use super::*;
    use prodex_domain::{
        BudgetLimit, BudgetSnapshot, CallId, IdempotencyKey, ReservationId, ReservationRequest,
        TenantId, UsageAmount, VirtualKeyId,
    };
    use prodex_storage::{AtomicReservationCommand, BudgetStorageScope, TenantStorageKey};
    use std::path::Path;
    use std::sync::{Arc, Barrier};

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

    fn sqlite_reservation_command(tenant_id: TenantId) -> AtomicReservationCommand {
        let call_id = CallId::new();
        let reservation_id = ReservationId::new();
        AtomicReservationCommand {
            storage_key: TenantStorageKey::virtual_key(tenant_id, VirtualKeyId::new()),
            idempotency_key: IdempotencyKey::from_call_reservation(call_id, reservation_id),
            snapshot: BudgetSnapshot::default(),
            limit: BudgetLimit::new(1_000, 10_000),
            request: ReservationRequest {
                tenant_id,
                call_id,
                reservation_id,
                estimate: UsageAmount::new(25, 250),
            },
            created_at_unix_ms: 1_000,
            ttl_ms: 60_000,
        }
    }

    fn sqlite_reservation_state(
        path: &Path,
        command: &AtomicReservationCommand,
    ) -> ((i64, i64, i64, i64), i64, i64) {
        let conn = runtime_gateway_sqlite_open(path).expect("sqlite database should open");
        let counter = conn
            .query_row(
                "SELECT reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros FROM prodex_budget_counters WHERE tenant_id = ?1 AND storage_scope = ?2",
                rusqlite::params![
                    command.request.tenant_id.to_string(),
                    command.storage_key.storage_scope()
                ],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("budget counter should exist");
        let reservations: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM prodex_reservations WHERE tenant_id = ?1",
                rusqlite::params![command.request.tenant_id.to_string()],
                |row| row.get(0),
            )
            .expect("reservation count should load");
        let ledger: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM prodex_usage_ledger WHERE tenant_id = ?1",
                rusqlite::params![command.request.tenant_id.to_string()],
                |row| row.get(0),
            )
            .expect("ledger count should load");
        (counter, reservations, ledger)
    }

    fn sqlite_reserve(
        path: &Path,
        command: &AtomicReservationCommand,
    ) -> Result<(), RuntimeGatewayDurableReservationError> {
        let storage = prodex_storage_sqlite::plan_sqlite_atomic_reservation(command.clone())
            .expect("sqlite reservation plan should be valid");
        runtime_gateway_sqlite_reserve_usage(path, &storage, command)
    }

    #[test]
    fn sqlite_reservation_replay_requires_an_exact_record_and_never_mutates_on_conflict() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-reservation-replay-{}",
            RequestId::new()
        ));
        std::fs::create_dir_all(&root).expect("test root should be created");
        let path = root.join("state.sqlite");
        runtime_gateway_sqlite_create_current_schema_for_tests(&path)
            .expect("sqlite schema fixture should be created");

        let tenant_id = TenantId::new();
        let conn = runtime_gateway_sqlite_open(&path).expect("sqlite database should open");
        conn.execute(
            "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms) VALUES (?1, 'tenant', 1, 1)",
            rusqlite::params![tenant_id.to_string()],
        )
        .expect("tenant row should insert");
        drop(conn);

        let command = sqlite_reservation_command(tenant_id);
        sqlite_reserve(&path, &command).expect("first reservation should apply");
        let applied = sqlite_reservation_state(&path, &command);
        sqlite_reserve(&path, &command).expect("exact reservation replay should succeed");
        assert_eq!(sqlite_reservation_state(&path, &command), applied);

        let storage = prodex_storage_sqlite::plan_sqlite_atomic_reservation(command.clone())
            .expect("sqlite reservation plan should be valid");
        let mut tenant_mismatch = command.clone();
        tenant_mismatch.request.tenant_id = TenantId::new();
        assert!(matches!(
            runtime_gateway_sqlite_reserve_usage(&path, &storage, &tenant_mismatch),
            Err(RuntimeGatewayDurableReservationError::Conflict)
        ));

        let conn = runtime_gateway_sqlite_open(&path).expect("sqlite database should open");
        conn.execute(
            "UPDATE prodex_budget_counters SET virtual_key_id = NULL
             WHERE tenant_id = ?1 AND storage_scope = ?2",
            rusqlite::params![tenant_id.to_string(), command.storage_key.storage_scope()],
        )
        .expect("counter tamper should apply");
        assert!(matches!(
            sqlite_reserve(&path, &command),
            Err(RuntimeGatewayDurableReservationError::Conflict)
        ));
        conn.execute(
            "UPDATE prodex_budget_counters SET virtual_key_id = ?1
             WHERE tenant_id = ?2 AND storage_scope = ?3",
            rusqlite::params![
                command.storage_key.virtual_key_id.map(|id| id.to_string()),
                tenant_id.to_string(),
                command.storage_key.storage_scope(),
            ],
        )
        .expect("counter repair should apply");
        conn.execute(
            "UPDATE prodex_usage_ledger SET tokens = 26
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'reserved'",
            rusqlite::params![
                tenant_id.to_string(),
                command.request.reservation_id.to_string()
            ],
        )
        .expect("ledger tamper should apply");
        assert!(matches!(
            sqlite_reserve(&path, &command),
            Err(RuntimeGatewayDurableReservationError::Conflict)
        ));
        conn.execute(
            "UPDATE prodex_usage_ledger SET tokens = 25
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND event_kind = 'reserved'",
            rusqlite::params![
                tenant_id.to_string(),
                command.request.reservation_id.to_string()
            ],
        )
        .expect("ledger repair should apply");

        let mut idempotency_mismatch = command.clone();
        idempotency_mismatch.idempotency_key = IdempotencyKey::new("different-idempotency-key")
            .expect("test idempotency key should be valid");

        let mut call_mismatch = command.clone();
        call_mismatch.request.call_id = CallId::new();
        call_mismatch.idempotency_key = IdempotencyKey::from_call_reservation(
            call_mismatch.request.call_id,
            call_mismatch.request.reservation_id,
        );

        let mut reservation_mismatch = command.clone();
        reservation_mismatch.request.reservation_id = ReservationId::new();
        reservation_mismatch.idempotency_key = IdempotencyKey::from_call_reservation(
            reservation_mismatch.request.call_id,
            reservation_mismatch.request.reservation_id,
        );

        let mut virtual_key_mismatch = command.clone();
        virtual_key_mismatch.storage_key =
            TenantStorageKey::virtual_key(tenant_id, VirtualKeyId::new());

        let mut scope_mismatch = command.clone();
        scope_mismatch.storage_key = TenantStorageKey::budget_group(
            tenant_id,
            command.storage_key.virtual_key_id.expect("virtual key"),
            BudgetStorageScope::from_digest([7; 32]),
        );

        let mut token_mismatch = command.clone();
        token_mismatch.request.estimate = UsageAmount::new(26, 250);

        let mut cost_mismatch = command.clone();
        cost_mismatch.request.estimate = UsageAmount::new(25, 251);

        let mut created_at_mismatch = command.clone();
        created_at_mismatch.created_at_unix_ms += 1;

        let mut ttl_mismatch = command.clone();
        ttl_mismatch.ttl_ms += 1;

        for (name, candidate) in [
            ("idempotency", idempotency_mismatch),
            ("call", call_mismatch),
            ("reservation", reservation_mismatch),
            ("virtual key", virtual_key_mismatch),
            ("storage scope", scope_mismatch),
            ("reserved tokens", token_mismatch),
            ("reserved cost", cost_mismatch),
            ("created at", created_at_mismatch),
            ("ttl", ttl_mismatch),
        ] {
            let error = sqlite_reserve(&path, &candidate);
            assert!(
                matches!(error, Err(RuntimeGatewayDurableReservationError::Conflict)),
                "{name} mismatch should be a typed conflict: {error:?}"
            );
            assert_eq!(sqlite_reservation_state(&path, &command), applied);
        }

        std::fs::remove_dir_all(root).expect("test root should clean up");
    }

    #[test]
    fn sqlite_concurrent_exact_reservation_replays_apply_once() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-reservation-concurrent-replay-{}",
            RequestId::new()
        ));
        std::fs::create_dir_all(&root).expect("test root should be created");
        let path = root.join("state.sqlite");
        runtime_gateway_sqlite_create_current_schema_for_tests(&path)
            .expect("sqlite schema fixture should be created");

        let tenant_id = TenantId::new();
        let conn = runtime_gateway_sqlite_open(&path).expect("sqlite database should open");
        conn.execute(
            "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms) VALUES (?1, 'tenant', 1, 1)",
            rusqlite::params![tenant_id.to_string()],
        )
        .expect("tenant row should insert");
        drop(conn);

        let command = sqlite_reservation_command(tenant_id);
        let barrier = Arc::new(Barrier::new(2));
        let handles = (0..2)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                let path = path.clone();
                let command = command.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    sqlite_reserve(&path, &command)
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            assert!(
                handle
                    .join()
                    .expect("reservation worker should finish")
                    .is_ok(),
                "exact concurrent replay should not reject"
            );
        }

        assert_eq!(
            sqlite_reservation_state(&path, &command),
            ((25, 250, 0, 0), 1, 1)
        );
        std::fs::remove_dir_all(root).expect("test root should clean up");
    }
}
