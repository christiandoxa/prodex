use super::local_rewrite::{
    RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY, RuntimeGatewayLedgerScope,
    RuntimeGatewayVirtualKeyUsageDelta, RuntimeLocalRewriteProxyShared,
    schedule_runtime_gateway_virtual_key_usage_save,
};
use super::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_open;
use super::local_rewrite_gateway_budget::{
    runtime_gateway_budget_group_rejection, runtime_gateway_budget_storage_key,
};
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_distributed_rate_limit::{
    runtime_gateway_distributed_rate_limit_admission,
    runtime_gateway_distributed_rate_limit_required, runtime_gateway_durable_budget_enforced,
    runtime_gateway_local_admission_usage,
};
use super::local_rewrite_gateway_key_store_backend::{
    runtime_gateway_postgres_load_key_store, runtime_gateway_redis_load_key_store,
    runtime_gateway_sqlite_load_key_store,
};
use super::local_rewrite_gateway_reservation::runtime_gateway_postgres_reserve_usage;
use super::local_rewrite_gateway_sqlite_utils::runtime_gateway_sqlite_u64_to_i64;
use super::local_rewrite_gateway_store_file::runtime_gateway_virtual_key_store_file_load;
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayVirtualKeyEntry, RuntimeGatewayVirtualKeySource,
    RuntimeGatewayVirtualKeyStoreFile, runtime_gateway_virtual_key_effective_id,
    runtime_gateway_virtual_key_entry_from_stored,
};
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_seconds;
use super::provider_bridge::runtime_provider_gateway_cost_for_request;
use super::*;
use anyhow::Result;
use prodex_domain::{
    BudgetLimit, BudgetSnapshot, CallId, IdempotencyKey, RequestId, ReservationRecord,
    ReservationRequest, TenantId, UsageAmount,
};
use prodex_provider_core::{calculate_cost_microusd, estimate_request_input_tokens};
use rusqlite::OptionalExtension;
use std::path::Path;
use std::str::FromStr;

const RUNTIME_GATEWAY_RESERVATION_TTL_MS: u64 = 60_000;

pub(super) enum RuntimeGatewayDurableReservationError {
    Rejected(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection),
    Failed,
}

#[derive(Clone)]
pub(super) struct RuntimeGatewayDurableReservationState {
    pub(super) storage_key: prodex_storage::TenantStorageKey,
    pub(super) record: ReservationRecord,
}

pub(super) fn runtime_gateway_virtual_key_entries_is_empty(
    shared: &RuntimeLocalRewriteProxyShared,
) -> bool {
    shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.is_empty())
        .unwrap_or(false)
}

struct RuntimeGatewayVirtualKeySnapshot {
    active_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    configured_count: usize,
}

fn runtime_gateway_virtual_key_snapshot(
    entries: std::sync::LockResult<std::sync::MutexGuard<'_, Vec<RuntimeGatewayVirtualKeyEntry>>>,
) -> Result<RuntimeGatewayVirtualKeySnapshot, runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection>
{
    let entries = entries.map_err(|_| {
        runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
    })?;
    Ok(RuntimeGatewayVirtualKeySnapshot {
        active_keys: entries
            .iter()
            .filter(|entry| !entry.disabled)
            .map(|entry| entry.key.clone())
            .collect(),
        configured_count: entries.len(),
    })
}

pub(super) fn runtime_gateway_virtual_key_entries_from_sources(
    policy_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> Result<Vec<RuntimeGatewayVirtualKeyEntry>> {
    let mut entries = policy_keys
        .into_iter()
        .map(|key| RuntimeGatewayVirtualKeyEntry {
            virtual_key_id: None,
            tenant_id: key.tenant_id.clone(),
            key,
            source: RuntimeGatewayVirtualKeySource::Policy,
            created_at_epoch: None,
            updated_at_epoch: None,
            disabled: false,
        })
        .collect::<Vec<_>>();
    let mut seen = entries
        .iter()
        .map(|entry| entry.key.name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    for record in runtime_gateway_virtual_key_store_load_strict(state_store, log_path)?.keys {
        let key_name = record.name.trim().to_string();
        if key_name.is_empty() {
            continue;
        }
        let normalized = key_name.to_ascii_lowercase();
        if seen.iter().any(|seen| seen == &normalized) {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_store_duplicate_ignored",
                    [runtime_proxy_log_field("key", key_name)],
                ),
            );
            continue;
        }
        let Some(entry) = runtime_gateway_virtual_key_entry_from_stored(&record) else {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_store_invalid_hash",
                    [runtime_proxy_log_field("key", &key_name)],
                ),
            );
            anyhow::bail!("gateway virtual key store contains invalid token hash for {key_name}");
        };
        seen.push(normalized);
        entries.push(entry);
    }
    Ok(entries)
}

pub(super) fn runtime_gateway_virtual_key_store_load_strict(
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let path = state_store.key_store_path();
    let store = match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => runtime_gateway_sqlite_load_key_store(path),
        RuntimeGatewayStateStore::Postgres { url, tls, .. } => {
            runtime_gateway_postgres_load_key_store(url, tls)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_redis_load_key_store(url, RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY)
        }
        RuntimeGatewayStateStore::File { .. } => runtime_gateway_virtual_key_store_file_load(path)
            .map_err(|err| anyhow::anyhow!(err.to_string())),
    }
    .inspect_err(|_err| {
        runtime_proxy_log_to_path(
            log_path,
            &runtime_proxy_structured_log_message(
                "gateway_virtual_key_store_load_failed",
                [
                    runtime_proxy_log_field("backend", state_store.label()),
                    runtime_proxy_log_field("error_kind", "gateway_key_store_persistence_failed"),
                ],
            ),
        );
    })?;
    Ok(runtime_gateway_prepare_virtual_key_store(store))
}

pub(super) fn runtime_gateway_virtual_key_store_load(
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> RuntimeGatewayVirtualKeyStoreFile {
    runtime_gateway_virtual_key_store_load_strict(state_store, log_path).unwrap_or_default()
}

pub(super) fn runtime_gateway_request_header_rejection(
    request_id: u64,
    request: &tiny_http::Request,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection> {
    if path_without_query(request.url()) == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH {
        return None;
    }
    let snapshot = match runtime_gateway_virtual_key_snapshot(shared.gateway_virtual_keys.lock()) {
        Ok(snapshot) => snapshot,
        Err(rejection) => {
            runtime_proxy_log_to_path(
                &shared.runtime_shared.log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_state_unavailable",
                    [runtime_proxy_log_field("request", request_id.to_string())],
                ),
            );
            return Some(rejection);
        }
    };
    if snapshot.active_keys.is_empty() && snapshot.configured_count > 0 {
        return Some(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::MissingOrInvalidToken);
    }
    let headers = request
        .headers()
        .iter()
        .map(|header| (header.field.to_string(), header.value.as_str().to_string()))
        .collect::<Vec<_>>();
    runtime_proxy_crate::runtime_gateway_virtual_key_from_headers(&headers, &snapshot.active_keys)
        .err()
}

fn runtime_gateway_prepare_virtual_key_store(
    mut store: RuntimeGatewayVirtualKeyStoreFile,
) -> RuntimeGatewayVirtualKeyStoreFile {
    store.canonicalize_for_active_state();
    store.sort_for_rendering();
    store
}

pub(super) fn runtime_gateway_virtual_key_admission(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<Option<String>, runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection> {
    if path_without_query(&captured.path_and_query) == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH
    {
        return Ok(None);
    }
    let snapshot = match runtime_gateway_virtual_key_snapshot(shared.gateway_virtual_keys.lock()) {
        Ok(snapshot) => snapshot,
        Err(rejection) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_virtual_key_state_unavailable",
                    [runtime_proxy_log_field("request", request_id.to_string())],
                ),
            );
            return Err(rejection);
        }
    };
    let active_keys = snapshot.active_keys;
    if active_keys.is_empty() && snapshot.configured_count > 0 {
        return Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::MissingOrInvalidToken);
    }
    let key = match runtime_proxy_crate::runtime_gateway_virtual_key_from_headers(
        &captured.headers,
        &active_keys,
    ) {
        Ok(Some(key)) => key,
        Ok(None) => return Ok(None),
        Err(rejection) => return Err(rejection),
    };
    let model = runtime_proxy_crate::runtime_gateway_request_model(&captured.body)
        .unwrap_or_else(|| "unknown".to_string());
    let input_tokens = estimate_request_input_tokens(&captured.body);
    let reserved_tokens = runtime_proxy_crate::runtime_gateway_estimated_tokens(&captured.body);
    let reserved_output_tokens = reserved_tokens.saturating_sub(input_tokens);
    let route_load = match shared.gateway_route_load.lock() {
        Ok(load) => load.clone(),
        Err(_) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_route_load_state_unavailable",
                    [runtime_proxy_log_field("request", request_id.to_string())],
                ),
            );
            return Err(
                runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable,
            );
        }
    };
    let cost = runtime_provider_gateway_cost_for_request(
        shared.provider.bridge_kind(),
        &shared.gateway_route_aliases,
        &route_load,
        request_id,
        &captured.body,
        &model,
    );
    let estimated_cost_microusd =
        calculate_cost_microusd(Some(input_tokens), Some(reserved_output_tokens), cost);
    let minute_epoch = runtime_proxy_crate::runtime_gateway_minute_epoch();
    let entry = match shared.gateway_virtual_keys.lock() {
        Ok(entries) => entries
            .iter()
            .find(|entry| entry.key.name.eq_ignore_ascii_case(&key.name))
            .cloned(),
        Err(_) => {
            return Err(
                runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable,
            );
        }
    };
    let durable_budget_enforced =
        runtime_gateway_durable_budget_enforced(shared, key, entry.as_ref());
    let Ok(usage) = shared.gateway_usage.usage.lock() else {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_virtual_key_usage_state_unavailable",
                [runtime_proxy_log_field("request", request_id.to_string())],
            ),
        );
        return Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable);
    };
    let mut usage = Some(usage);
    let usage_snapshot = usage
        .as_deref()
        .ok_or(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)?;
    if !durable_budget_enforced
        && let Some(rejection) = runtime_gateway_budget_group_rejection(
            key,
            &active_keys,
            usage_snapshot,
            estimated_cost_microusd,
        )
    {
        return Err(rejection);
    }
    let key_usage = runtime_gateway_local_admission_usage(
        key,
        usage_snapshot.get(&key.name),
        durable_budget_enforced,
    );
    let admission = runtime_proxy_crate::runtime_gateway_virtual_key_admission(
        key,
        Some(&key_usage),
        &captured.body,
        estimated_cost_microusd,
        minute_epoch,
    )?;
    if runtime_gateway_distributed_rate_limit_required(shared, key) {
        drop(usage.take());
        runtime_gateway_distributed_rate_limit_admission(
            shared,
            key,
            entry.as_ref(),
            &admission,
            minute_epoch,
        )?;
    }
    let call_id = CallId::new();
    let durable_reservation = if let Some(entry) = entry.as_ref() {
        match runtime_gateway_try_durable_reservation(shared, key, entry, &admission, call_id) {
            Ok(state) => state,
            Err(error) => {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "gateway_virtual_key_durable_reservation_failed",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("key", admission.key_name.as_str()),
                            runtime_proxy_log_field(
                                "error_kind",
                                match error {
                                    RuntimeGatewayDurableReservationError::Rejected(_) => {
                                        "gateway_reservation_rejected"
                                    }
                                    RuntimeGatewayDurableReservationError::Failed => {
                                        "gateway_reservation_storage_failed"
                                    }
                                },
                            ),
                            runtime_proxy_log_field("backend", shared.gateway_state_store.label()),
                        ],
                    ),
                );
                return Err(match error {
                    RuntimeGatewayDurableReservationError::Rejected(rejection) => rejection,
                    RuntimeGatewayDurableReservationError::Failed => {
                        runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
                    }
                });
            }
        }
    } else {
        None
    };
    if usage.is_none() {
        usage = Some(shared.gateway_usage.usage.lock().map_err(|_| {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
        })?);
    }
    let Some(usage_map) = usage.as_deref_mut() else {
        return Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable);
    };
    let usage_entry = usage_map.entry(admission.key_name.clone()).or_default();
    runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
        usage_entry,
        &admission,
        minute_epoch,
    );
    drop(usage);
    if let Ok(mut request_ids) = shared.gateway_usage.request_ids.lock() {
        request_ids.insert(request_id);
    }
    let typed_request_id = format!("prodex-{}", RequestId::new());
    if let Ok(mut typed_request_ids) = shared.gateway_usage.typed_request_ids.lock() {
        typed_request_ids.insert(request_id, typed_request_id.clone());
    }
    if let Some(state) = durable_reservation
        && let Ok(mut durable_reservations) = shared.gateway_usage.durable_reservations.lock()
    {
        durable_reservations.insert(request_id, state);
    }
    let call_id = format!("prodex-{call_id}");
    if let Ok(mut call_ids) = shared.gateway_usage.call_ids.lock() {
        call_ids.insert(request_id, call_id.clone());
    }
    if let Ok(mut ledger_scopes) = shared.gateway_usage.ledger_scopes.lock() {
        ledger_scopes.insert(
            request_id,
            RuntimeGatewayLedgerScope {
                key_name: admission.key_name.clone(),
                tenant_id: key.tenant_id.clone(),
            },
        );
    }
    schedule_runtime_gateway_virtual_key_usage_save(
        shared,
        RuntimeGatewayVirtualKeyUsageDelta {
            request_id,
            typed_request_id,
            call_id,
            key_name: admission.key_name.clone(),
            tenant_id: key.tenant_id.clone(),
            team_id: key.team_id.clone(),
            project_id: key.project_id.clone(),
            user_id: key.user_id.clone(),
            budget_id: key.budget_id.clone(),
            model: model.clone(),
            minute_epoch,
            input_tokens: admission.input_tokens,
            reserved_tokens: admission.reserved_tokens,
            estimated_cost_microusd: admission.estimated_cost_microusd,
            created_at_epoch: runtime_gateway_unix_epoch_seconds(),
        },
    );
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_virtual_key_admitted",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("key", admission.key_name.as_str()),
                runtime_proxy_log_field("model", model.as_str()),
                runtime_proxy_log_field("input_tokens", admission.input_tokens.to_string()),
                runtime_proxy_log_field(
                    "estimated_cost_microusd",
                    admission
                        .estimated_cost_microusd
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                ),
            ],
        ),
    );
    Ok(Some(admission.key_name))
}

fn runtime_gateway_try_durable_reservation(
    shared: &RuntimeLocalRewriteProxyShared,
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
    entry: &RuntimeGatewayVirtualKeyEntry,
    admission: &runtime_proxy_crate::RuntimeGatewayVirtualKeyAdmission,
    call_id: CallId,
) -> Result<Option<RuntimeGatewayDurableReservationState>, RuntimeGatewayDurableReservationError> {
    let (RuntimeGatewayStateStore::Sqlite { .. } | RuntimeGatewayStateStore::Postgres { .. }) =
        &shared.gateway_state_store
    else {
        return Ok(None);
    };
    let Some(virtual_key_id) = runtime_gateway_virtual_key_effective_id(entry) else {
        return Ok(None);
    };
    let Some(tenant_id) = key
        .tenant_id
        .as_deref()
        .and_then(|tenant_id| TenantId::from_str(tenant_id).ok())
    else {
        return Ok(None);
    };
    let request = ReservationRequest {
        tenant_id,
        call_id,
        reservation_id: prodex_domain::ReservationId::new(),
        estimate: UsageAmount::new(
            admission.reserved_tokens,
            admission.estimated_cost_microusd.unwrap_or_default(),
        ),
    };
    let durable_store = match &shared.gateway_state_store {
        RuntimeGatewayStateStore::Sqlite { .. } => prodex_storage::DurableStoreKind::Sqlite,
        RuntimeGatewayStateStore::Postgres { .. } => prodex_storage::DurableStoreKind::Postgres,
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            return Ok(None);
        }
    };
    let created_at_unix_ms = runtime_gateway_unix_epoch_millis();
    let idempotency_key =
        IdempotencyKey::from_call_reservation(request.call_id, request.reservation_id);
    let storage_key = runtime_gateway_budget_storage_key(tenant_id, virtual_key_id, key);
    let command = prodex_storage::AtomicReservationCommand {
        storage_key,
        idempotency_key,
        snapshot: BudgetSnapshot::default(),
        limit: BudgetLimit::new(
            i64::MAX as u64,
            key.budget_microusd.unwrap_or(i64::MAX as u64),
        )
        .with_max_requests(key.request_budget.unwrap_or(i64::MAX as u64)),
        request,
        created_at_unix_ms,
        ttl_ms: RUNTIME_GATEWAY_RESERVATION_TTL_MS,
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
        ) => runtime_gateway_sqlite_reserve_usage(path, &storage, &command)?,
        (
            RuntimeGatewayStateStore::Postgres { .. },
            prodex_application::ApplicationAtomicReservationStoragePlan::Postgres(_),
        ) => runtime_gateway_postgres_reserve_usage(shared, command.clone())?,
        _ => {}
    }
    let record = ReservationRecord::from_request(
        command.request,
        command.created_at_unix_ms,
        command.ttl_ms,
    )
    .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?;
    Ok(Some(RuntimeGatewayDurableReservationState {
        storage_key,
        record,
    }))
}

fn runtime_gateway_sqlite_reserve_usage(
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
            tenant_id, reservation_id, call_id, virtual_key_id, idempotency_key,
            reserved_tokens, reserved_cost_micros, created_at_unix_ms, expires_at_unix_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
        rusqlite::params![
            tenant_id,
            reservation_id,
            call_id,
            virtual_key_id,
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

pub(super) fn runtime_local_rewrite_request_is_authorized(
    request: &tiny_http::Request,
    auth_token_hash: &runtime_proxy_crate::LocalBridgeBearerTokenHash,
) -> bool {
    let path = path_without_query(request.url());
    if path == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH
        || path.ends_with("/prodex/gateway/admin")
    {
        return true;
    }
    request.headers().iter().any(|header| {
        header.field.equiv("Authorization")
            && auth_token_hash.verify_authorization_header(header.value.as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_launch::proxy_startup::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_create_current_schema_for_tests;
    use crate::runtime_launch::proxy_startup::local_rewrite_gateway_store_types::{
        RuntimeGatewayScimUser, runtime_gateway_virtual_key_store_version,
    };
    use std::sync::{Arc, Barrier};

    #[test]
    fn key_store_load_failure_log_uses_stable_error_without_path_details() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("prodex-key-store-load-log-redaction-{nonce}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test root should be created");
        let key_store_path = root.join("gateway-virtual-keys.json");
        std::fs::create_dir_all(&key_store_path).expect("key store path directory should exist");
        let log_path = root.join("runtime.log");
        let state_store = RuntimeGatewayStateStore::File {
            key_store_path: key_store_path.clone(),
            usage_path: root.join("gateway-virtual-key-usage.json"),
            ledger_path: root.join("gateway-billing-ledger.jsonl"),
        };

        let store = runtime_gateway_virtual_key_store_load(&state_store, &log_path);
        assert!(store.keys.is_empty());
        let runtime_log = std::fs::read_to_string(&log_path).expect("runtime log should exist");
        assert!(runtime_log.contains("gateway_virtual_key_store_load_failed"));
        assert!(runtime_log.contains("error_kind=gateway_key_store_persistence_failed"));
        assert!(!runtime_log.contains("gateway-virtual-keys.json"));
        assert!(!runtime_log.contains(&root.display().to_string()));
        assert!(!runtime_log.contains("Is a directory"));
    }

    #[test]
    fn key_store_load_filters_malformed_scim_rows_from_active_state() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("prodex-key-store-scim-filter-{nonce}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test root should be created");
        let key_store_path = root.join("gateway-virtual-keys.json");
        let log_path = root.join("runtime.log");
        let valid = RuntimeGatewayScimUser {
            id: prodex_domain::PrincipalId::new().to_string(),
            user_name: "alice@example.com".to_string(),
            external_id: None,
            display_name: None,
            active: true,
            role: Some("admin".to_string()),
            tenant_id: Some(String::new()),
            team_id: None,
            project_id: None,
            user_id: Some(prodex_domain::PrincipalId::new().to_string()),
            budget_id: None,
            allowed_key_prefixes: vec!["team-a-".to_string()],
            created_at_epoch: 1,
            updated_at_epoch: 2,
        };
        let mut malformed = valid.clone();
        malformed.id = prodex_domain::PrincipalId::new().to_string();
        malformed.user_name = "mallory@example.com".to_string();
        malformed.tenant_id = Some(" tenant-a ".to_string());
        let store = RuntimeGatewayVirtualKeyStoreFile {
            version: runtime_gateway_virtual_key_store_version(),
            keys: Vec::new(),
            scim_users: vec![malformed, valid.clone()],
        };
        std::fs::write(&key_store_path, serde_json::to_vec_pretty(&store).unwrap()).unwrap();
        let state_store = RuntimeGatewayStateStore::File {
            key_store_path,
            usage_path: root.join("gateway-virtual-key-usage.json"),
            ledger_path: root.join("gateway-billing-ledger.jsonl"),
        };

        let loaded = runtime_gateway_virtual_key_store_load(&state_store, &log_path);

        assert_eq!(loaded.scim_users.len(), 1);
        assert_eq!(loaded.scim_users[0].user_name, valid.user_name);
        assert_eq!(loaded.scim_users[0].tenant_id, None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn admission_cost_estimate_includes_reserved_output_tokens() {
        let body = br#"{"model":"gpt-5.4","input":"hello from prodex","max_output_tokens":17}"#;
        let input_tokens = estimate_request_input_tokens(body);
        let reserved_tokens = runtime_proxy_crate::runtime_gateway_estimated_tokens(body);
        let reserved_output_tokens = reserved_tokens.saturating_sub(input_tokens);
        let cost = prodex_provider_core::ProviderModelCost {
            input_cost_per_million_microusd: Some(1_000_000),
            output_cost_per_million_microusd: Some(2_000_000),
        };

        let estimated_cost =
            calculate_cost_microusd(Some(input_tokens), Some(reserved_output_tokens), cost);

        assert_eq!(input_tokens, 5);
        assert_eq!(reserved_tokens, 22);
        assert_eq!(reserved_output_tokens, 17);
        assert_eq!(estimated_cost, Some(39));
    }

    #[test]
    fn durable_budget_admission_ignores_stale_local_spend_and_requests() {
        let key = runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "admin-managed-key".to_string(),
            tenant_id: Some(TenantId::new().to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("token"),
            allowed_models: Vec::new(),
            budget_microusd: Some(100),
            request_budget: Some(10),
            rpm_limit: Some(10),
            tpm_limit: Some(1_000),
        };
        let usage = runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
            minute_epoch: 10,
            requests_this_minute: 1,
            tokens_this_minute: 1,
            requests_total: 1,
            spend_microusd: 100,
        };
        let body = br#"{"model":"gpt-5.4","input":"hello"}"#;

        assert!(matches!(
            runtime_proxy_crate::runtime_gateway_virtual_key_admission(
                &key,
                Some(&usage),
                body,
                Some(1),
                10,
            ),
            Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::BudgetExceeded)
        ));

        let local_usage = runtime_gateway_local_admission_usage(&key, Some(&usage), true);
        let admitted = runtime_proxy_crate::runtime_gateway_virtual_key_admission(
            &key,
            Some(&local_usage),
            body,
            Some(1),
            10,
        )
        .expect("durable-backed budget should not be blocked by stale local usage");

        assert_eq!(admitted.key_name, "admin-managed-key");
        assert_eq!(local_usage.requests_total, 0);
        assert_eq!(local_usage.requests_this_minute, 1);
        assert_eq!(local_usage.spend_microusd, 0);
    }

    #[test]
    fn durable_budget_admission_defers_request_budget_to_durable_store() {
        let key = runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "admin-managed-key".to_string(),
            tenant_id: Some(TenantId::new().to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("token"),
            allowed_models: Vec::new(),
            budget_microusd: Some(100),
            request_budget: Some(1),
            rpm_limit: None,
            tpm_limit: None,
        };
        let usage = runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
            requests_total: 1,
            spend_microusd: 100,
            ..Default::default()
        };
        let local_usage = runtime_gateway_local_admission_usage(&key, Some(&usage), true);

        assert_eq!(local_usage.requests_total, 0);
        runtime_proxy_crate::runtime_gateway_virtual_key_admission(
            &key,
            Some(&local_usage),
            br#"{"model":"gpt-5.4","input":"hello"}"#,
            Some(1),
            10,
        )
        .expect("durable-backed request budget should not be blocked by stale local usage");
    }

    #[test]
    fn sqlite_durable_reservation_rejects_second_concurrent_claim_on_same_budget_scope() {
        let root = std::env::temp_dir().join(format!(
            "prodex-sqlite-durable-reserve-{}",
            RequestId::new()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test root should be created");
        let path = root.join("state.sqlite");
        runtime_gateway_sqlite_create_current_schema_for_tests(&path)
            .expect("sqlite schema fixture should be created");

        let tenant_id = TenantId::new();
        let virtual_key_id = prodex_domain::VirtualKeyId::new();
        let tenant_id_text = tenant_id.to_string();
        let conn = runtime_gateway_sqlite_open(&path).expect("sqlite database should open");
        conn.execute(
            "INSERT INTO prodex_tenants (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![tenant_id_text, "tenant", 1_i64, 1_i64],
        )
        .expect("tenant row should insert");
        drop(conn);

        let make_command = move || {
            let call_id = CallId::new();
            let reservation_id = prodex_domain::ReservationId::new();
            prodex_storage::AtomicReservationCommand {
                storage_key: prodex_storage::TenantStorageKey::virtual_key(
                    tenant_id,
                    virtual_key_id,
                ),
                idempotency_key: IdempotencyKey::from_call_reservation(call_id, reservation_id),
                snapshot: BudgetSnapshot::default(),
                limit: BudgetLimit::new(u64::MAX, 42),
                request: ReservationRequest {
                    tenant_id,
                    call_id,
                    reservation_id,
                    estimate: UsageAmount::new(22, 42),
                },
                created_at_unix_ms: 1_000,
                ttl_ms: RUNTIME_GATEWAY_RESERVATION_TTL_MS,
            }
        };
        let barrier = Arc::new(Barrier::new(2));
        let run = |barrier: Arc<Barrier>| {
            let path = path.clone();
            std::thread::spawn(move || {
                let command = make_command();
                let plan = prodex_application::plan_application_atomic_reservation(
                    prodex_application::ApplicationAtomicReservationRequest {
                        durable_store: prodex_storage::DurableStoreKind::Sqlite,
                        reservation: command.clone(),
                    },
                )
                .expect("sqlite reservation plan should build");
                let prodex_application::ApplicationAtomicReservationStoragePlan::Sqlite(storage) =
                    plan.storage
                else {
                    panic!("expected sqlite storage plan");
                };
                barrier.wait();
                runtime_gateway_sqlite_reserve_usage(&path, &storage, &command)
            })
        };

        let first = run(Arc::clone(&barrier));
        let second = run(barrier);
        let results = [
            first.join().expect("first thread should finish"),
            second.join().expect("second thread should finish"),
        ];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(RuntimeGatewayDurableReservationError::Rejected(
                        runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::BudgetExceeded
                    ))
                ))
                .count(),
            1
        );

        let conn = runtime_gateway_sqlite_open(&path).expect("sqlite database should reopen");
        let reservation_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM prodex_reservations", [], |row| {
                row.get(0)
            })
            .expect("reservation count should load");
        let reserved_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM prodex_usage_ledger WHERE event_kind = 'reserved'",
                [],
                |row| row.get(0),
            )
            .expect("reserved ledger count should load");
        let (reserved_tokens, reserved_cost_micros): (i64, i64) = conn
            .query_row(
                "SELECT reserved_tokens, reserved_cost_micros FROM prodex_budget_counters",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("budget counters should load");
        assert_eq!(reservation_rows, 1);
        assert_eq!(reserved_rows, 1);
        assert_eq!(reserved_tokens, 22);
        assert_eq!(reserved_cost_micros, 42);

        let _ = std::fs::remove_dir_all(root);
    }
}
