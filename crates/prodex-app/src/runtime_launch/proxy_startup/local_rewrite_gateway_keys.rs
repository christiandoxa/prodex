use super::local_rewrite::{
    RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY, RuntimeGatewayLedgerScope,
    RuntimeGatewayVirtualKeyUsageDelta, RuntimeLocalRewriteProxyShared,
    schedule_runtime_gateway_virtual_key_usage_save,
};
use super::local_rewrite_application_data_plane::runtime_gateway_application_data_plane_admission;
use super::local_rewrite_gateway_admission::{
    RUNTIME_GATEWAY_REALTIME_SESSION_MAX_MILLIS, RUNTIME_GATEWAY_REALTIME_SESSION_MAX_TOKENS,
    RUNTIME_GATEWAY_RESERVATION_TTL_MS, RuntimeGatewayRealtimeAccountingPlan,
    RuntimeGatewayVirtualKeyAdmissionFailure, RuntimeGatewayVirtualKeyAdmissionOutcome,
    runtime_gateway_application_admission_rejection,
    runtime_gateway_application_admission_without_virtual_key,
    runtime_gateway_conversation_namespace,
};
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_distributed_rate_limit::runtime_gateway_distributed_rate_limit_admission;
use super::local_rewrite_gateway_key_store_backend::{
    runtime_gateway_postgres_load_key_store, runtime_gateway_redis_load_key_store,
    runtime_gateway_sqlite_load_key_store,
};
pub(super) use super::local_rewrite_gateway_reservation::{
    RuntimeGatewayDurableReservationError, RuntimeGatewayDurableReservationState,
};
use super::local_rewrite_gateway_reservation::{
    runtime_gateway_limit_reservation_cost, runtime_gateway_limit_reservation_tokens,
    runtime_gateway_try_durable_reservation,
};
use super::local_rewrite_gateway_store_file::runtime_gateway_virtual_key_store_file_load;
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayVirtualKeyEntry, RuntimeGatewayVirtualKeySource,
    RuntimeGatewayVirtualKeyStoreFile, runtime_gateway_apply_scim_policy_attributes,
    runtime_gateway_principal_policy_attributes, runtime_gateway_virtual_key_effective_id,
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
use prodex_domain::{CallId, TenantId};
use prodex_gateway_core::{
    GatewayVirtualKeyAdmissionRequest, GatewayVirtualKeyReservationContext,
    GatewayVirtualKeyUsageEntry, apply_gateway_virtual_key_usage_update,
};
use prodex_provider_core::estimate_request_input_tokens;

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
            group_ids: Vec::new(),
            department_id: None,
            created_at_epoch: None,
            updated_at_epoch: None,
            disabled: false,
        })
        .collect::<Vec<_>>();
    let mut seen = entries
        .iter()
        .map(|entry| entry.key.name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let store = runtime_gateway_virtual_key_store_load_strict(state_store, log_path)?;
    for record in &store.keys {
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
        let Some(entry) = runtime_gateway_virtual_key_entry_from_stored(record) else {
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
    runtime_gateway_apply_scim_policy_attributes(&mut entries, &store.scim_users);
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
    reservation_ttl_ms: u64,
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
            ttl_ms: input.reservation_ttl_ms,
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
    let realtime = runtime_proxy_crate::is_runtime_realtime_websocket_path(path_without_query(
        &captured.path_and_query,
    ));
    let request_model = if realtime {
        Some(
            shared
                .runtime_shared
                .runtime_config
                .gemini
                .live_model
                .clone()
                .unwrap_or_else(|| {
                    super::local_rewrite_gemini_live::runtime_gemini_live_default_model()
                        .to_string()
                }),
        )
    } else {
        runtime_proxy_crate::runtime_gateway_request_model(&captured.body)
    };
    let model = request_model
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let input_tokens = if realtime {
        0
    } else {
        estimate_request_input_tokens(&captured.body)
    };
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
    let (mut reserved_tokens, hard_limit_reservation) = runtime_gateway_limit_reservation_tokens(
        realtime,
        shared
            .runtime_shared
            .runtime_config
            .governance
            .mode
            .is_enforcing(),
        key.tpm_limit.is_some() || key.budget_microusd.is_some(),
        shared.provider.bridge_kind().provider_id(),
        &pricing_model,
        &captured.body,
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
    let realtime_token_limit = if realtime {
        let current = usage_snapshot.get(&key.name).cloned().unwrap_or_default();
        let used = if current.minute_epoch == minute_epoch {
            current.tokens_this_minute
        } else {
            0
        };
        let available = key
            .tpm_limit
            .map(|limit| limit.saturating_sub(used))
            .unwrap_or(RUNTIME_GATEWAY_REALTIME_SESSION_MAX_TOKENS);
        let limit = available.min(RUNTIME_GATEWAY_REALTIME_SESSION_MAX_TOKENS);
        if limit == 0 {
            return Err(
                runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::TpmLimitExceeded.into(),
            );
        }
        reserved_tokens = limit;
        Some(limit)
    } else {
        None
    };
    let estimated_cost_microusd = runtime_gateway_limit_reservation_cost(
        hard_limit_reservation,
        key.budget_microusd.is_some(),
        reserved_tokens,
        input_tokens,
        cost,
    )?;
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
            reservation_ttl_ms: if realtime {
                RUNTIME_GATEWAY_REALTIME_SESSION_MAX_MILLIS
            } else {
                RUNTIME_GATEWAY_RESERVATION_TTL_MS
            },
        })?;
    let admission = virtual_key_plan.gateway.admission.clone();
    let usage_update = virtual_key_plan.gateway.usage_update;
    let command =
        virtual_key_plan.gateway.reservation.clone().ok_or(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable,
        )?;
    let principal_attributes = runtime_gateway_principal_policy_attributes(key, entry.as_ref())
        .map_err(|_| {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
        })?;
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
        namespace: Some(runtime_gateway_conversation_namespace(
            &tenant_id,
            "virtual-key",
            &admission.key_name,
        )),
        application,
        realtime_accounting: realtime_token_limit.map(|token_limit| {
            RuntimeGatewayRealtimeAccountingPlan {
                token_limit,
                model,
                cost,
            }
        }),
    })
}

#[cfg(test)]
#[path = "local_rewrite_gateway_keys_tests.rs"]
mod tests;
