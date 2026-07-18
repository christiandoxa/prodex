use super::local_rewrite::{
    RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY, RuntimeGatewayLedgerScope,
    RuntimeGatewayVirtualKeyUsageDelta, RuntimeLocalRewriteProxyShared,
    schedule_runtime_gateway_virtual_key_usage_save,
};
use super::local_rewrite_application_data_plane::{
    RuntimeGatewayApplicationAdmission, RuntimeGatewayApplicationDataPlaneError,
    runtime_gateway_application_data_plane_admission,
};
use super::local_rewrite_gateway_backend_connection::runtime_gateway_sqlite_open;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_distributed_rate_limit::runtime_gateway_distributed_rate_limit_admission;
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
use super::local_rewrite_gateway_usage::runtime_gateway_try_reserve_usage_delta;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_seconds;
use super::provider_bridge::{
    runtime_provider_gateway_cost_for_request, runtime_provider_gateway_pricing_model,
};
use super::*;
use anyhow::Result;
use prodex_application::{
    ApplicationInspectionPlan, ApplicationVirtualKeyAdmissionError,
    ApplicationVirtualKeyAdmissionPlan, plan_application_virtual_key_admission,
};
use prodex_domain::{
    ApprovalId, ApprovalState, BudgetLimit, BudgetSnapshot, CallId, IdempotencyKey,
    PrincipalPolicyAttributes, RequestId, ReservationRecord, ReservationRequest, TenantId,
    UsageAmount,
};
use prodex_gateway_core::{
    GatewayVirtualKeyAdmissionRequest, GatewayVirtualKeyReservationContext,
    GatewayVirtualKeyUsageEntry, apply_gateway_virtual_key_usage_update,
};
use prodex_provider_core::{calculate_cost_microusd, estimate_request_input_tokens};
use rusqlite::OptionalExtension;
use std::path::Path;

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

pub(super) struct RuntimeGatewayVirtualKeyAdmissionOutcome {
    pub(super) namespace: Option<String>,
    pub(super) application: RuntimeGatewayApplicationAdmission,
}

pub(super) struct RuntimeGatewayVirtualKeyAdmissionFailure {
    pub(super) rejection: runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection,
    pub(super) approval: Option<(ApprovalId, ApprovalState)>,
}

impl From<runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection>
    for RuntimeGatewayVirtualKeyAdmissionFailure
{
    fn from(rejection: runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection) -> Self {
        Self {
            rejection,
            approval: None,
        }
    }
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

pub(super) fn runtime_gateway_request_header_virtual_key(
    request_id: u64,
    request: &super::local_rewrite_request::RuntimeLocalRewriteRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<
    Option<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection,
> {
    if path_without_query(request.url()) == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH {
        return Ok(None);
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
            return Err(rejection);
        }
    };
    if snapshot.active_keys.is_empty() && snapshot.configured_count > 0 {
        return Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::MissingOrInvalidToken);
    }
    runtime_proxy_crate::runtime_gateway_virtual_key_from_headers(
        request.headers(),
        &snapshot.active_keys,
    )
    .map(|key| key.cloned())
}

fn runtime_gateway_prepare_virtual_key_store(
    mut store: RuntimeGatewayVirtualKeyStoreFile,
) -> RuntimeGatewayVirtualKeyStoreFile {
    store.canonicalize_for_active_state();
    store.sort_for_rendering();
    store
}

struct RuntimeGatewayVirtualKeyPlanInput<'a> {
    shared: &'a RuntimeLocalRewriteProxyShared,
    key: &'a runtime_proxy_crate::RuntimeGatewayVirtualKey,
    active_keys: &'a [runtime_proxy_crate::RuntimeGatewayVirtualKey],
    usage:
        &'a std::collections::BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
    entry: Option<&'a RuntimeGatewayVirtualKeyEntry>,
    tenant_id: TenantId,
    call_id: CallId,
    model: Option<String>,
    input_tokens: u64,
    reserved_tokens: u64,
    estimated_cost_microusd: Option<u64>,
    minute_epoch: u64,
}

fn runtime_gateway_application_virtual_key_admission(
    input: RuntimeGatewayVirtualKeyPlanInput<'_>,
) -> Result<
    ApplicationVirtualKeyAdmissionPlan,
    runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection,
> {
    let durable_store = match &input.shared.gateway_state_store {
        RuntimeGatewayStateStore::Sqlite { .. } => Some(prodex_storage::DurableStoreKind::Sqlite),
        RuntimeGatewayStateStore::Postgres { .. } => {
            Some(prodex_storage::DurableStoreKind::Postgres)
        }
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => None,
    };
    let grouped_usage = input
        .active_keys
        .iter()
        .map(|key| GatewayVirtualKeyUsageEntry {
            policy: runtime_proxy_crate::runtime_gateway_virtual_key_policy(key),
            usage: input.usage.get(&key.name).cloned().unwrap_or_default(),
        })
        .collect();
    plan_application_virtual_key_admission(GatewayVirtualKeyAdmissionRequest {
        policy: runtime_proxy_crate::runtime_gateway_virtual_key_policy(input.key),
        usage: input
            .usage
            .get(&input.key.name)
            .cloned()
            .unwrap_or_default(),
        grouped_usage,
        model: input.model,
        input_tokens: input.input_tokens,
        reserved_tokens: input.reserved_tokens,
        estimated_cost_microusd: input.estimated_cost_microusd,
        minute_epoch: input.minute_epoch,
        reservation: Some(GatewayVirtualKeyReservationContext {
            tenant_id: input.tenant_id,
            virtual_key_id: input
                .entry
                .and_then(runtime_gateway_virtual_key_effective_id),
            call_id: input.call_id,
            reservation_id: prodex_domain::ReservationId::new(),
            durable_store,
            created_at_unix_ms: runtime_gateway_unix_epoch_millis(),
            ttl_ms: RUNTIME_GATEWAY_RESERVATION_TTL_MS,
        }),
        distributed_rate_limit: input.shared.gateway_redis_rate_limit_executor.is_some(),
        now_unix_ms: runtime_gateway_unix_epoch_millis(),
    })
    .map_err(|error| match error {
        ApplicationVirtualKeyAdmissionError::Gateway(error) => error.into(),
        ApplicationVirtualKeyAdmissionError::DistributedRateLimit(_) => {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
        }
    })
}

pub(super) fn runtime_gateway_virtual_key_admission(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    network_zone: prodex_domain::NetworkZone,
    authorized: &prodex_application::ApplicationAuthorizedRequestContext<'_>,
    inspection: &ApplicationInspectionPlan,
) -> Result<RuntimeGatewayVirtualKeyAdmissionOutcome, RuntimeGatewayVirtualKeyAdmissionFailure> {
    if path_without_query(&captured.path_and_query) == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH
    {
        return Err(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable.into(),
        );
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
            return Err(rejection.into());
        }
    };
    let active_keys = snapshot.active_keys;
    if active_keys.is_empty() && snapshot.configured_count > 0 {
        return Err(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::MissingOrInvalidToken.into(),
        );
    }
    let key = match runtime_proxy_crate::runtime_gateway_virtual_key_from_headers(
        &captured.headers,
        &active_keys,
    ) {
        Ok(Some(key)) => key,
        Ok(None) => {
            return runtime_gateway_application_admission_without_virtual_key(
                request_id,
                captured,
                shared,
                network_zone,
                authorized,
                inspection,
            );
        }
        Err(rejection) => return Err(rejection.into()),
    };
    let authorized_tenant = authorized.tenant_context().map(|tenant| tenant.tenant_id);
    if let (Some(authorized_tenant), Some(key_tenant)) = (
        authorized_tenant,
        key.tenant_id
            .as_deref()
            .and_then(|value| value.parse::<TenantId>().ok()),
    ) && authorized_tenant != key_tenant
    {
        return Err(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable.into(),
        );
    }
    let request_model = runtime_proxy_crate::runtime_gateway_request_model(&captured.body);
    let model = request_model
        .clone()
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
                runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
                    .into(),
            );
        }
    };
    let pricing_model = runtime_provider_gateway_pricing_model(
        &shared.gateway_route_aliases,
        &route_load,
        request_id,
        &captured.body,
        &model,
    );
    let governed_cost = authorized_tenant.and_then(|tenant_id| {
        shared
            .governed_provider_registry
            .load_full()
            .snapshot_for(tenant_id)
            .and_then(|snapshot| snapshot.reservation_cost_for_model(&pricing_model))
    });
    let cost = runtime_provider_gateway_cost_for_request(
        shared.provider.bridge_kind(),
        &shared.gateway_route_aliases,
        &route_load,
        request_id,
        &captured.body,
        &model,
        governed_cost,
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
                runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
                    .into(),
            );
        }
    };
    let Ok(usage) = shared.gateway_usage.usage.lock() else {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_virtual_key_usage_state_unavailable",
                [runtime_proxy_log_field("request", request_id.to_string())],
            ),
        );
        return Err(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable.into(),
        );
    };
    let mut usage = Some(usage);
    let usage_snapshot = usage
        .as_deref()
        .ok_or(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)?;
    let tenant_id = authorized_tenant
        .ok_or(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)?;
    let typed_request_id = authorized.request().request_id();
    let call_id = CallId::new();
    let virtual_key_plan =
        runtime_gateway_application_virtual_key_admission(RuntimeGatewayVirtualKeyPlanInput {
            shared,
            key,
            active_keys: &active_keys,
            usage: usage_snapshot,
            entry: entry.as_ref(),
            tenant_id,
            call_id,
            model: request_model,
            input_tokens,
            reserved_tokens,
            estimated_cost_microusd,
            minute_epoch,
        })?;
    let admission = virtual_key_plan.gateway.admission.clone();
    let usage_update = virtual_key_plan.gateway.usage_update;
    let command =
        virtual_key_plan.gateway.reservation.clone().ok_or(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable,
        )?;
    let principal_attributes = PrincipalPolicyAttributes::new(
        key.team_id.as_deref(),
        key.project_id.as_deref(),
        key.user_id.as_deref(),
    )
    .map_err(|_| runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)?;
    let application = runtime_gateway_application_data_plane_admission(
        authorized,
        captured,
        shared,
        network_zone,
        principal_attributes,
        command.clone(),
        inspection.clone(),
    )
    .map_err(runtime_gateway_application_admission_rejection)?;
    let Some(usage_delta_permit) = runtime_gateway_try_reserve_usage_delta(shared) else {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_accounting_queue_saturated",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("queue", "usage"),
                ],
            ),
        );
        return Err(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable.into(),
        );
    };
    let Some(reconciliation_permit) = shared.gateway_usage.reconciliation.try_reserve() else {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_accounting_queue_saturated",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("queue", "reconciliation"),
                ],
            ),
        );
        return Err(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable.into(),
        );
    };
    if let Some(rate_limit) = virtual_key_plan.distributed_rate_limit.as_ref() {
        drop(usage.take());
        runtime_gateway_distributed_rate_limit_admission(shared, rate_limit)?;
    }
    let durable_reservation = if virtual_key_plan.gateway.durable_reservation {
        match runtime_gateway_try_durable_reservation(shared, &command, &application) {
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
                    RuntimeGatewayDurableReservationError::Rejected(rejection) => rejection.into(),
                    RuntimeGatewayDurableReservationError::Failed => {
                        runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
                            .into()
                    }
                });
            }
        }
    } else {
        None
    };
    if usage.is_none() {
        usage = Some(
            shared
                .gateway_usage
                .usage
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }
    let Some(usage_map) = usage.as_deref_mut() else {
        return Err(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable.into(),
        );
    };
    let usage_entry = usage_map.entry(admission.key_name.clone()).or_default();
    apply_gateway_virtual_key_usage_update(usage_entry, usage_update);
    drop(usage);
    shared
        .gateway_usage
        .request_ids
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(request_id);
    let typed_request_id = format!("prodex-{typed_request_id}");
    shared
        .gateway_usage
        .typed_request_ids
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(request_id, typed_request_id.clone());
    if let Some(state) = durable_reservation {
        shared
            .gateway_usage
            .durable_reservations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(request_id, state);
    }
    let call_id = format!("prodex-{call_id}");
    shared
        .gateway_usage
        .call_ids
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(request_id, call_id.clone());
    shared
        .gateway_usage
        .ledger_scopes
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            request_id,
            RuntimeGatewayLedgerScope {
                key_name: admission.key_name.clone(),
                tenant_id: key.tenant_id.clone(),
            },
        );
    shared
        .gateway_usage
        .reconciliation
        .commit(request_id, reconciliation_permit);
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
        usage_delta_permit,
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
    Ok(RuntimeGatewayVirtualKeyAdmissionOutcome {
        namespace: Some(admission.key_name),
        application,
    })
}

fn runtime_gateway_application_admission_without_virtual_key(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    network_zone: prodex_domain::NetworkZone,
    authorized: &prodex_application::ApplicationAuthorizedRequestContext<'_>,
    inspection: &ApplicationInspectionPlan,
) -> Result<RuntimeGatewayVirtualKeyAdmissionOutcome, RuntimeGatewayVirtualKeyAdmissionFailure> {
    let Some(tenant) = authorized.tenant_context() else {
        return Ok(RuntimeGatewayVirtualKeyAdmissionOutcome {
            namespace: None,
            application: RuntimeGatewayApplicationAdmission::compatibility_anonymous(
                authorized.request().route(),
                captured,
                shared,
                inspection.clone(),
            )
            .map_err(|_| {
                RuntimeGatewayVirtualKeyAdmissionFailure::from(
                    runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable,
                )
            })?,
        });
    };
    let call_id = CallId::new();
    let reservation_id = prodex_domain::ReservationId::new();
    let estimate = UsageAmount::new(
        runtime_proxy_crate::runtime_gateway_estimated_tokens(&captured.body).max(1),
        0,
    );
    let command = prodex_storage::AtomicReservationCommand {
        storage_key: prodex_storage::TenantStorageKey::tenant(tenant.tenant_id),
        idempotency_key: IdempotencyKey::from_call_reservation(call_id, reservation_id),
        snapshot: BudgetSnapshot::default(),
        limit: BudgetLimit::new(u64::MAX, u64::MAX),
        request: ReservationRequest {
            tenant_id: tenant.tenant_id,
            call_id,
            reservation_id,
            estimate,
        },
        created_at_unix_ms: runtime_gateway_unix_epoch_millis(),
        ttl_ms: RUNTIME_GATEWAY_RESERVATION_TTL_MS,
    };
    let application = runtime_gateway_application_data_plane_admission(
        authorized,
        captured,
        shared,
        network_zone,
        PrincipalPolicyAttributes::default(),
        command,
        inspection.clone(),
    )
    .map_err(|error| {
        let rejection = runtime_gateway_application_admission_rejection(error);
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_application_admission_failed",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("reason", rejection.rejection.code()),
                ],
            ),
        );
        rejection
    })?;
    Ok(RuntimeGatewayVirtualKeyAdmissionOutcome {
        namespace: None,
        application,
    })
}

fn runtime_gateway_application_admission_rejection(
    error: RuntimeGatewayApplicationDataPlaneError,
) -> RuntimeGatewayVirtualKeyAdmissionFailure {
    match error {
        RuntimeGatewayApplicationDataPlaneError::GovernanceDenied => {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::GovernanceDenied.into()
        }
        RuntimeGatewayApplicationDataPlaneError::GovernanceApprovalRequired {
            approval_id,
            state,
        } => RuntimeGatewayVirtualKeyAdmissionFailure {
            rejection:
                runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::GovernanceApprovalRequired,
            approval: Some((approval_id, state)),
        },
        RuntimeGatewayApplicationDataPlaneError::GovernanceSessionRequired => {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::GovernanceSessionRequired.into()
        }
        RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider => {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::NoEligibleProvider.into()
        }
        RuntimeGatewayApplicationDataPlaneError::Execution(_)
        | RuntimeGatewayApplicationDataPlaneError::MissingPrincipal
        | RuntimeGatewayApplicationDataPlaneError::RouteUnavailable
        | RuntimeGatewayApplicationDataPlaneError::ProviderRoute(_)
        | RuntimeGatewayApplicationDataPlaneError::TraceContext(_)
        | RuntimeGatewayApplicationDataPlaneError::Admission(_)
        | RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable => {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable.into()
        }
    }
}

fn runtime_gateway_try_durable_reservation(
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

pub(super) fn runtime_local_rewrite_request_is_authorized(
    request: &super::local_rewrite_request::RuntimeLocalRewriteRequest,
    auth_token_hash: &runtime_proxy_crate::LocalBridgeBearerTokenHash,
) -> bool {
    let path = path_without_query(request.url());
    if path == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH
        || path.ends_with("/prodex/gateway/admin")
    {
        return true;
    }
    request.headers().iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("authorization")
            && auth_token_hash.verify_authorization_header(value)
    })
}

#[cfg(test)]
#[path = "local_rewrite_gateway_keys_tests.rs"]
mod tests;
