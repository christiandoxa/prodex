use super::super::copilot_instructions::runtime_copilot_init_current_workspace_custom_instructions;
use super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
mod context;
mod listener_worker;
pub(super) use self::context::{
    RuntimeLocalRewriteProcessServices, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteRequestContext,
};
use self::listener_worker::spawn_runtime_local_rewrite_listener_worker;
#[cfg(test)]
pub(crate) use super::local_rewrite_constraints::start_runtime_gateway_rewrite_proxy;
#[cfg(test)]
pub(crate) use super::local_rewrite_constraints::start_runtime_local_rewrite_proxy;
pub(crate) use super::local_rewrite_constraints::{
    start_runtime_gateway_rewrite_proxy_with_runtime_config,
    start_runtime_local_rewrite_proxy_with_harness,
};
use super::local_rewrite_copilot::runtime_copilot_oauth_pool_from_provider;
use super::local_rewrite_gateway_admin_auth::runtime_gateway_run_oidc_background_refresh_loop;
pub(crate) use super::local_rewrite_gateway_config::{
    RuntimeGatewayAdminRole, RuntimeGatewayAdminToken, RuntimeGatewayBrowserConfig,
    RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewayOidcConfig, RuntimeGatewaySsoConfig, RuntimeGatewayStateStore,
    RuntimeGatewayWorkloadIdentityConfig, runtime_gateway_postgres_repository,
    runtime_gateway_redis_rate_limit_executor,
};
use super::local_rewrite_gateway_credentials::{
    RuntimeGatewayCredentialRefreshPlan, RuntimeGatewayCredentialState,
    runtime_gateway_initial_credential_snapshot, runtime_gateway_pin_request_credentials,
    runtime_gateway_spawn_secret_refresh,
};
pub(super) use super::local_rewrite_gateway_guardrail_webhook::runtime_gateway_guardrail_webhook_block;
pub(super) use super::local_rewrite_gateway_keys::{
    RuntimeGatewayDurableReservationState, runtime_gateway_virtual_key_entries_from_sources,
    runtime_gateway_virtual_key_store_load_strict,
};
pub(super) use super::local_rewrite_gateway_ledger::runtime_gateway_billing_ledger_load;
pub(super) use super::local_rewrite_gateway_reconciliation_worker::{
    RuntimeGatewayReconciliationQueue, schedule_runtime_gateway_billing_ledger_reconcile,
};
use super::local_rewrite_gateway_reservation_recovery::spawn_runtime_gateway_reservation_recovery_worker;
#[cfg(test)]
pub(super) use super::local_rewrite_gateway_usage::runtime_gateway_virtual_key_usage_apply_deltas;
pub(super) use super::local_rewrite_gateway_usage::{
    RuntimeGatewayPendingUsageDelta, runtime_gateway_virtual_key_usage_load_strict,
    schedule_runtime_gateway_virtual_key_usage_save,
};
pub(super) use super::local_rewrite_gateway_usage_backend::RuntimeGatewayVirtualKeyUsageDelta;
pub(super) use super::local_rewrite_gateway_util::runtime_gateway_generate_virtual_key_token;
use super::local_rewrite_gemini::runtime_gemini_oauth_pool_from_provider;
use super::local_rewrite_gemini_live::spawn_runtime_gemini_live_sidecar;
pub(super) use super::local_rewrite_model_memory::{
    RuntimeLocalRewriteModelMemoryState, runtime_local_rewrite_model_selection,
};
pub(crate) use super::local_rewrite_options::{
    RuntimeGatewaySecret, RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
    RuntimeProjectedProviderCredential,
};
use super::local_rewrite_pipeline::run_runtime_local_rewrite_pipeline;
use super::local_rewrite_request::RuntimeLocalRewriteRequest;
pub(super) use super::local_rewrite_upstream::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult,
};
use super::provider_bridge::runtime_provider_label;
use crate::presidio_runtime::runtime_governed_presidio_redaction_config;
use crate::proxy_config::{
    build_runtime_upstream_async_http_client, build_runtime_upstream_async_http_compact_client,
    runtime_upstream_proxy_mode_label,
};
use crate::quota_support::validate_credential_free_http_url;
use crate::runtime_background::{
    RuntimeProxyMarkerGuard, initialize_runtime_probe_refresh_queue,
    register_runtime_proxy_persistence_mode,
};
use crate::runtime_config::RuntimeConfig;
use crate::runtime_core_shared::{
    initialize_runtime_proxy_log_path_from_config, runtime_proxy_log_to_path,
};
use crate::runtime_proxy::{
    build_runtime_proxy_json_error_response, register_runtime_presidio_redaction_proxy_state,
    register_runtime_smart_context_proxy_state,
};
use crate::runtime_state_shared::{
    RuntimeContinuationStatuses, RuntimeRotationProxyShared, RuntimeRotationState,
};
use crate::{RuntimeRotationProxy, runtime_proxy_log, runtime_proxy_request_sequence_seed};
use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use prodex_provider_core::provider_adapter;
use prodex_runtime_state::{RuntimeProxyLaneAdmission, RuntimeProxyLaneLimits};
use runtime_proxy_crate::{
    RuntimeProxyRequest, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tiny_http::Server as TinyServer;
use tokio::runtime::Builder as TokioRuntimeBuilder;
pub(crate) const RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH: &str = "/v1";
pub(super) const RUNTIME_LOCAL_REWRITE_PROFILE: &str = "local";
pub(super) const RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY: &str = "prodex:gateway:virtual_keys";
pub(super) const RUNTIME_GATEWAY_REDIS_KEY_STORE_LOCK: &str = "prodex:gateway:virtual_keys:lock";
pub(super) const RUNTIME_GATEWAY_REDIS_LEDGER_KEY: &str = "prodex:gateway:billing_ledger";
pub(super) const RUNTIME_GATEWAY_REDIS_LEDGER_LOCK: &str = "prodex:gateway:billing_ledger:lock";
const RUNTIME_GATEWAY_BACKGROUND_TASK_LIMIT: usize = 32;
pub(super) const RUNTIME_GATEWAY_CONVERSATION_NAMESPACE_HEADER: &str =
    "x-prodex-internal-conversation-namespace";
pub(super) const RUNTIME_GATEWAY_SCIM_USER_SCHEMA: &str =
    "urn:ietf:params:scim:schemas:core:2.0:User";
pub(super) const RUNTIME_GATEWAY_SCIM_PRODEX_SCHEMA: &str =
    "urn:prodex:params:scim:schemas:gateway:2.0:User";

#[derive(Clone)]
pub(super) enum RuntimeGovernanceAuthority {
    Sqlite {
        path: PathBuf,
        tenant_ids: Arc<Mutex<BTreeSet<prodex_domain::TenantId>>>,
    },
    Postgres {
        repository: prodex_storage_postgres_runtime::PostgresRepository,
        runtime: Arc<tokio::runtime::Runtime>,
        tenant_ids: Arc<Mutex<BTreeSet<prodex_domain::TenantId>>>,
    },
}

impl RuntimeGovernanceAuthority {
    pub(super) fn tenant_ids(
        &self,
    ) -> std::result::Result<Vec<prodex_domain::TenantId>, prodex_storage::GovernanceRepositoryError>
    {
        let tenant_ids = match self {
            Self::Sqlite { tenant_ids, .. } | Self::Postgres { tenant_ids, .. } => tenant_ids,
        };
        tenant_ids
            .lock()
            .map(|tenant_ids| tenant_ids.iter().copied().collect())
            .map_err(|_| prodex_storage::GovernanceRepositoryError::Database)
    }

    pub(super) fn commit_for_tenant<T>(
        &self,
        tenant_id: prodex_domain::TenantId,
        operation: impl FnOnce() -> std::result::Result<T, prodex_storage::GovernanceRepositoryError>,
    ) -> std::result::Result<T, prodex_storage::GovernanceRepositoryError> {
        let tenant_ids = match self {
            Self::Sqlite { tenant_ids, .. } | Self::Postgres { tenant_ids, .. } => tenant_ids,
        };
        let mut tenant_ids = tenant_ids
            .lock()
            .map_err(|_| prodex_storage::GovernanceRepositoryError::Database)?;
        if !tenant_ids.contains(&tenant_id)
            && tenant_ids.len()
                >= crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS
        {
            return Err(prodex_storage::GovernanceRepositoryError::SnapshotUnavailable);
        }
        let result = operation()?;
        tenant_ids.insert(tenant_id);
        Ok(result)
    }

    fn merge_tenant_ids(
        &self,
        discovered: impl IntoIterator<Item = prodex_domain::TenantId>,
    ) -> std::result::Result<(), prodex_storage::GovernanceRepositoryError> {
        let tenant_ids = match self {
            Self::Sqlite { tenant_ids, .. } | Self::Postgres { tenant_ids, .. } => tenant_ids,
        };
        let mut tenant_ids = tenant_ids
            .lock()
            .map_err(|_| prodex_storage::GovernanceRepositoryError::Database)?;
        for tenant_id in discovered {
            if !tenant_ids.contains(&tenant_id)
                && tenant_ids.len()
                    >= crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS
            {
                return Err(prodex_storage::GovernanceRepositoryError::SnapshotUnavailable);
            }
            tenant_ids.insert(tenant_id);
        }
        Ok(())
    }
}

impl RuntimeLocalRewriteRequestContext {
    pub(super) fn swap_committed_governance_artifact_kind(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: prodex_storage::GovernanceArtifactKind,
        artifact: &[u8],
    ) -> Result<()> {
        match kind {
            prodex_storage::GovernanceArtifactKind::Policy => {
                let snapshot =
                    crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                        artifact,
                        self.runtime_shared.runtime_config.governance.mode,
                    )?;
                let next = self
                    .governance_snapshot
                    .load_full()
                    .with_tenant_snapshot(tenant_id, snapshot)?;
                self.governance_snapshot.store(Arc::new(next));
            }
            prodex_storage::GovernanceArtifactKind::ClassificationRules => {
                let snapshot = super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
                    tenant_id,
                    artifact,
                )?;
                let next = self
                    .classification_rules
                    .load_full()
                    .with_tenant_snapshot(tenant_id, snapshot)?;
                self.classification_rules.store(Arc::new(next));
            }
            prodex_storage::GovernanceArtifactKind::ProviderRegistry => {
                let snapshot = super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
                    artifact,
                    &self.provider,
                    self.provider_credential.as_ref(),
                    self.runtime_shared.runtime_config.governance.mode,
                )?;
                let next = self
                    .governed_provider_registry
                    .load_full()
                    .with_tenant_snapshot(tenant_id, snapshot)?;
                self.governed_provider_registry.store(Arc::new(next));
            }
            prodex_storage::GovernanceArtifactKind::RoutingScores => {
                let snapshot = super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(artifact)?;
                let next = self
                    .governed_routing_scores
                    .load_full()
                    .with_tenant_snapshot(tenant_id, snapshot)?;
                self.governed_routing_scores.store(Arc::new(next));
            }
        }
        Ok(())
    }

    fn conversation_store_for_request(
        &self,
        request: &RuntimeProxyRequest,
        store: &RuntimeDeepSeekConversationStore,
    ) -> RuntimeDeepSeekConversationStore {
        if self.allow_local_file_access {
            return store.clone();
        }
        let namespace = request
            .headers
            .iter()
            .find(|(name, _)| {
                name.eq_ignore_ascii_case(RUNTIME_GATEWAY_CONVERSATION_NAMESPACE_HEADER)
            })
            .map(|(_, value)| value.as_str())
            .unwrap_or("gateway");
        store.scoped(namespace)
    }

    pub(super) fn deepseek_conversations_for_request(
        &self,
        request: &RuntimeProxyRequest,
    ) -> RuntimeDeepSeekConversationStore {
        self.conversation_store_for_request(request, &self.deepseek_conversations)
    }

    pub(super) fn gemini_conversations_for_request(
        &self,
        request: &RuntimeProxyRequest,
    ) -> RuntimeDeepSeekConversationStore {
        self.conversation_store_for_request(request, &self.gemini_conversations)
    }
}

pub(super) type RuntimeLocalRewriteModelMemory = Arc<Mutex<RuntimeLocalRewriteModelMemoryState>>;
pub(super) type RuntimeGatewayRouteLoadState =
    Arc<Mutex<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelState>>>;

#[derive(Clone)]
pub(super) struct RuntimeGatewayOidcHttpCacheEntry {
    pub(super) fetched_at: std::time::Instant,
    pub(super) max_age: Option<std::time::Duration>,
    pub(super) stale_while_revalidate: Option<std::time::Duration>,
}

#[derive(Clone)]
pub(super) struct RuntimeGatewayVirtualKeyUsageState {
    pub(super) usage:
        Arc<Mutex<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>>>,
    pub(super) path: Option<PathBuf>,
    pub(super) save_in_flight: Arc<AtomicBool>,
    pub(super) save_dirty: Arc<AtomicBool>,
    pub(super) usage_slots: Arc<tokio::sync::Semaphore>,
    pub(super) pending_deltas: Arc<Mutex<Vec<RuntimeGatewayPendingUsageDelta>>>,
    pub(super) reconciliation: RuntimeGatewayReconciliationQueue,
    pub(super) request_ids: Arc<Mutex<BTreeSet<u64>>>,
    pub(super) typed_request_ids: Arc<Mutex<BTreeMap<u64, String>>>,
    pub(super) call_ids: Arc<Mutex<BTreeMap<u64, String>>>,
    pub(super) ledger_scopes: Arc<Mutex<BTreeMap<u64, RuntimeGatewayLedgerScope>>>,
    pub(super) durable_reservations:
        Arc<Mutex<BTreeMap<u64, RuntimeGatewayDurableReservationState>>>,
}

#[derive(Clone)]
pub(super) struct RuntimeGatewayLedgerScope {
    pub(super) key_name: String,
    pub(super) tenant_id: Option<String>,
}

pub(super) fn runtime_gateway_try_reserve_background_task(
    slots: &Arc<tokio::sync::Semaphore>,
) -> Option<tokio::sync::OwnedSemaphorePermit> {
    Arc::clone(slots).try_acquire_owned().ok()
}

pub(super) struct RuntimeGatewayBackgroundTaskGuard {
    count: Arc<AtomicUsize>,
}

impl RuntimeGatewayBackgroundTaskGuard {
    pub(super) fn new(shared: &RuntimeLocalRewriteProxyShared) -> Self {
        shared
            .gateway_background_task_count
            .fetch_add(1, Ordering::AcqRel);
        Self {
            count: Arc::clone(&shared.gateway_background_task_count),
        }
    }
}

impl Drop for RuntimeGatewayBackgroundTaskGuard {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(super) fn start_runtime_local_rewrite_proxy_with_file_access(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
    runtime_config: Arc<RuntimeConfig>,
    allow_local_file_access: bool,
    secret_refresh: Option<RuntimeGatewayCredentialRefreshPlan>,
    gateway_request_constraints: prodex_provider_core::ProviderRequestConstraintPolicy,
    resolved_harness: prodex_provider_core::ResolvedHarnessMode,
) -> Result<RuntimeRotationProxy> {
    validate_credential_free_http_url(&options.upstream_base_url, "runtime upstream base URL")?;
    let (server, listen_addr) = runtime_local_rewrite_server(options.preferred_listen_addr)?;
    let prepared = prepare_runtime_local_rewrite_application(
        options,
        runtime_config,
        allow_local_file_access,
        secret_refresh,
        gateway_request_constraints,
        resolved_harness,
        ("loopback", Some(listen_addr)),
    )?;
    let RuntimeLocalRewritePrepared {
        runtime_config,
        shared,
        shutdown,
        worker_count,
        secret_refresh,
        log_path,
        marker_guard,
    } = prepared;
    let RuntimeLocalRewriteWorkers {
        worker_threads,
        gemini_live_sidecar_addr,
    } = spawn_runtime_local_rewrite_workers(
        &shared,
        Some(&server),
        &shutdown,
        worker_count,
        secret_refresh,
        true,
    )?;
    Ok(RuntimeRotationProxy {
        runtime_config: Arc::clone(&runtime_config),
        server,
        draining: Arc::clone(&shared.gateway_draining),
        shutdown,
        worker_threads,
        accept_worker_count: worker_count,
        listen_addr,
        realtime_ws_sidecar_addr: gemini_live_sidecar_addr,
        realtime_ws_model: gemini_live_sidecar_addr.map(|_| {
            super::local_rewrite_gemini_live::runtime_gemini_live_default_model().to_string()
        }),
        log_path,
        active_request_count: Arc::clone(&shared.runtime_shared.active_request_count),
        #[cfg(test)]
        request_sequence: Arc::clone(&shared.runtime_shared.request_sequence),
        #[cfg(test)]
        lane_admission: shared.runtime_shared.lane_admission.clone(),
        #[cfg(test)]
        gateway_route_load: Some(Arc::clone(&shared.gateway_route_load)),
        #[cfg(test)]
        gateway_usage: Some(Arc::clone(&shared.gateway_usage.usage)),
        #[cfg(test)]
        gateway_side_effect_snapshot: Some(super::gateway_snapshot_handle(shared.clone())),
        owner_lock: None,
        _marker_guard: marker_guard,
    })
}

pub(super) struct RuntimeLocalRewritePrepared {
    pub(super) runtime_config: Arc<RuntimeConfig>,
    pub(super) shared: RuntimeLocalRewriteProxyShared,
    pub(super) shutdown: Arc<AtomicBool>,
    pub(super) worker_count: usize,
    pub(super) secret_refresh: Option<RuntimeGatewayCredentialRefreshPlan>,
    pub(super) log_path: PathBuf,
    pub(super) marker_guard: RuntimeProxyMarkerGuard,
}

pub(super) fn prepare_runtime_local_rewrite_application(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
    runtime_config: Arc<RuntimeConfig>,
    allow_local_file_access: bool,
    secret_refresh: Option<RuntimeGatewayCredentialRefreshPlan>,
    gateway_request_constraints: prodex_provider_core::ProviderRequestConstraintPolicy,
    resolved_harness: prodex_provider_core::ResolvedHarnessMode,
    transport: (&str, Option<std::net::SocketAddr>),
) -> Result<RuntimeLocalRewritePrepared> {
    let (transport, listen_addr) = transport;
    let RuntimeLocalRewriteProxyStartOptions {
        paths,
        state,
        upstream_base_url,
        provider,
        upstream_no_proxy,
        smart_context_enabled,
        presidio_redaction_enabled,
        model_context_window_tokens,
        preferred_listen_addr: _,
        gateway_auth_token_hash,
        gateway_admin_tokens,
        gateway_sso,
        gateway_state_store,
        gateway_virtual_keys,
        gateway_route_aliases,
        gateway_guardrails,
        gateway_guardrail_webhook,
        gateway_call_id_header,
        gateway_observability,
    } = options;
    validate_credential_free_http_url(&upstream_base_url, "runtime upstream base URL")?;
    let (provider, provider_credential) = provider.into_runtime_parts();
    let log_path = runtime_local_rewrite_log_path(&runtime_config);
    initialize_runtime_probe_refresh_queue(runtime_config.tuning.probe_refresh_worker_count);
    let worker_count = runtime_config.tuning.worker_count;
    let active_request_limit = runtime_config.tuning.active_request_limit;
    let lane_admission = RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
        responses: runtime_config.tuning.lane_limits.responses,
        compact: runtime_config.tuning.lane_limits.compact,
        websocket: runtime_config.tuning.lane_limits.websocket,
        standard: runtime_config.tuning.lane_limits.standard,
    });
    let async_runtime = Arc::new(
        TokioRuntimeBuilder::new_multi_thread()
            .worker_threads(runtime_config.tuning.async_worker_count)
            .enable_all()
            .build()
            .context("failed to build runtime local rewrite async runtime")?,
    );
    let runtime_shared = RuntimeRotationProxyShared {
        runtime_config: Arc::clone(&runtime_config),
        upstream_no_proxy,
        auto_redeem_enabled: false,
        async_client: build_runtime_upstream_async_http_client(true, &runtime_config)?,
        compact_client: build_runtime_upstream_async_http_compact_client(
            upstream_no_proxy,
            &runtime_config,
        )?,
        async_runtime,
        log_path: log_path.clone(),
        request_sequence: Arc::new(AtomicU64::new(runtime_proxy_request_sequence_seed(
            &log_path,
        ))),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission,
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: paths.clone(),
            state: state.clone(),
            upstream_base_url: upstream_base_url.clone(),
            include_code_review: false,
            current_profile: RUNTIME_LOCAL_REWRITE_PROFILE.to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
    };
    let gateway_postgres_repository =
        runtime_gateway_postgres_repository(&gateway_state_store, worker_count)?;
    let gateway_redis_rate_limit_executor =
        runtime_gateway_redis_rate_limit_executor(&gateway_state_store, &runtime_shared)?;
    let (
        governance_snapshot,
        classification_rules,
        governed_provider_registry,
        governed_routing_scores,
        governance_authority,
    ) = runtime_gateway_governance_authority(
        &runtime_config,
        &gateway_state_store,
        &gateway_admin_tokens,
        gateway_postgres_repository.as_ref(),
        &runtime_shared.async_runtime,
        &provider,
        provider_credential.as_ref(),
    )?;
    let marker_guard = RuntimeProxyMarkerGuard::new(&log_path);
    register_runtime_proxy_persistence_mode(&log_path, true);
    register_runtime_smart_context_proxy_state(
        &log_path,
        smart_context_enabled,
        model_context_window_tokens,
        Some(paths.root.join("runtime-smart-context-artifacts.json")),
    );
    register_runtime_presidio_redaction_proxy_state(
        &log_path,
        if presidio_redaction_enabled {
            Some(runtime_governed_presidio_redaction_config(
                paths,
                &runtime_config,
            )?)
        } else {
            None
        },
    )?;
    let bridge_kind = provider.bridge_kind();
    runtime_proxy_log_to_path(
        &log_path,
        &runtime_proxy_structured_log_message(
            "harness_resolution",
            [
                runtime_proxy_log_field("provider", runtime_provider_label(bridge_kind)),
                runtime_proxy_log_field("requested", resolved_harness.requested.to_string()),
                runtime_proxy_log_field("resolved", resolved_harness.effective.to_string()),
                runtime_proxy_log_field("source", resolved_harness.source.id()),
                runtime_proxy_log_field("reason", resolved_harness.reason_code()),
            ],
        ),
    );
    let openai_contract = provider_adapter(bridge_kind.provider_id());
    let gateway_virtual_key_store_path = gateway_state_store.key_store_path().to_path_buf();
    let gateway_virtual_key_usage_path = gateway_state_store.usage_path().to_path_buf();
    let gateway_virtual_key_entries = runtime_gateway_virtual_key_entries_from_sources(
        gateway_virtual_keys,
        &gateway_state_store,
        &log_path,
    )
    .context("failed to load gateway virtual key store")?;
    let gateway_virtual_key_usage =
        runtime_gateway_virtual_key_usage_load_strict(&gateway_state_store, &log_path)
            .context("failed to load gateway virtual key usage")?;
    let gateway_auth_required =
        gateway_auth_token_hash.is_some() || !gateway_virtual_key_entries.is_empty();
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime local rewrite application started transport={transport} listen_addr={} smart_context_enabled={smart_context_enabled} presidio_redaction_enabled={presidio_redaction_enabled} upstream_base_url={upstream_base_url} upstream_proxy_mode={} provider={} client_format={} upstream_format={} response_format={} endpoint={} auth_required={} virtual_keys={} route_aliases={} guardrail_blocked_keywords={} guardrail_blocked_output_keywords={} guardrail_allowed_models={} observability_sinks={}",
            listen_addr.map_or_else(|| "-".to_string(), |addr| addr.to_string()),
            runtime_upstream_proxy_mode_label(true),
            super::provider_bridge::runtime_provider_label(bridge_kind),
            openai_contract.client_request_format().label(),
            openai_contract.upstream_request_format().label(),
            openai_contract.response_format().label(),
            openai_contract.canonical_client_endpoint(),
            gateway_auth_required,
            gateway_virtual_key_entries.len(),
            gateway_route_aliases.len(),
            gateway_guardrails.blocked_keywords.len(),
            gateway_guardrails.blocked_output_keywords.len(),
            gateway_guardrails.allowed_models.len(),
            if gateway_observability.sinks.is_empty() {
                "-".to_string()
            } else {
                gateway_observability.sinks.join(",")
            }
        ),
    );
    let gemini_oauth_pool = runtime_gemini_oauth_pool_from_provider(&provider);
    let copilot_oauth_pool = runtime_copilot_oauth_pool_from_provider(&provider);
    if matches!(
        &provider,
        RuntimeLocalRewriteProviderOptions::Copilot { .. }
    ) {
        runtime_copilot_init_current_workspace_custom_instructions();
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    let gateway_virtual_keys = Arc::new(Mutex::new(gateway_virtual_key_entries));
    let gateway_credentials =
        RuntimeGatewayCredentialState::new(runtime_gateway_initial_credential_snapshot(
            super::local_rewrite_gateway_credentials::RuntimeGatewayCredentialRefreshCandidate {
                fingerprint: secret_refresh
                    .as_ref()
                    .map(|plan| plan.initial_fingerprint)
                    .unwrap_or([0; 32]),
                provider: provider.clone(),
                provider_credential: provider_credential.clone(),
                auth_token_hash: gateway_auth_token_hash.clone(),
                admin_tokens: gateway_admin_tokens.clone(),
                sso: gateway_sso.clone(),
                virtual_keys: Vec::new(),
                guardrail_webhook: gateway_guardrail_webhook.clone(),
                observability: gateway_observability.clone(),
            },
            Arc::clone(&gateway_virtual_keys),
        ));
    let process = Arc::new(RuntimeLocalRewriteProcessServices {
        runtime_shared: runtime_shared.clone(),
        mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        resolved_harness,
        deepseek_conversations: RuntimeDeepSeekConversationStore::default(),
        gemini_conversations: RuntimeDeepSeekConversationStore::default(),
        gemini_oauth_pool,
        copilot_oauth_pool,
        model_memory: Arc::new(Mutex::new(RuntimeLocalRewriteModelMemoryState::default())),
        governance_sessions: Default::default(),
        governance_audit_writer: Default::default(),
        governed_provider_registry,
        governed_routing_scores,
        classification_rules,
        governance_snapshot,
        governance_authority,
        api_key_cursor: Arc::new(AtomicUsize::new(0)),
        client: build_runtime_local_rewrite_http_client(&runtime_config)?,
        gateway_oidc_http_cache: Arc::new(Mutex::new(BTreeMap::new())),
        gateway_oidc_jwks_snapshot: Arc::new(arc_swap::ArcSwapOption::empty()),
        gateway_workload_jwks_snapshot: Arc::new(arc_swap::ArcSwapOption::empty()),
        gateway_browser: Default::default(),
        gateway_credentials,
        gateway_state_store,
        gateway_postgres_repository,
        gateway_redis_rate_limit_executor,
        gateway_policy_version: prodex_runtime_policy::runtime_policy_summary()
            .ok()
            .flatten()
            .map(|summary| summary.version),
        gateway_virtual_key_store_path,
        gateway_usage: runtime_local_rewrite_usage_state(
            gateway_virtual_key_usage,
            gateway_virtual_key_usage_path,
        ),
        gateway_route_aliases,
        gateway_request_constraints,
        gateway_route_load: Arc::new(Mutex::new(BTreeMap::new())),
        gateway_adaptive_routing: runtime_config.gateway.adaptive_routing,
        gateway_adaptive_quality: Default::default(),
        gateway_guardrails,
        gateway_call_id_header,
        gateway_observability_slots: Arc::new(tokio::sync::Semaphore::new(
            RUNTIME_GATEWAY_BACKGROUND_TASK_LIMIT,
        )),
        gateway_background_task_count: Arc::new(AtomicUsize::new(0)),
        allow_local_file_access,
        gateway_draining: Arc::new(AtomicBool::new(false)),
    });
    let shared = RuntimeLocalRewriteRequestContext {
        process,
        upstream_base_url,
        provider,
        provider_credential,
        governed_pricing: None,
        gateway_auth_token_hash,
        gateway_admin_tokens,
        gateway_sso,
        gateway_virtual_keys,
        gateway_guardrail_webhook,
        gateway_observability,
    };
    Ok(RuntimeLocalRewritePrepared {
        runtime_config,
        shared,
        shutdown,
        worker_count,
        secret_refresh,
        log_path,
        marker_guard,
    })
}

type RuntimeGatewayGovernanceAuthorityState = (
    Arc<ArcSwap<crate::runtime_governance::RuntimeGovernanceAuthoritySnapshotSet>>,
    Arc<ArcSwap<super::local_rewrite_classification_rules::RuntimeClassificationRulesSnapshotSet>>,
    Arc<ArcSwap<super::local_rewrite_provider_registry::RuntimeGatewayProviderRegistrySnapshotSet>>,
    Arc<ArcSwap<super::local_rewrite_provider_registry::RuntimeGatewayRoutingScoresSnapshotSet>>,
    Option<RuntimeGovernanceAuthority>,
);

fn runtime_gateway_governance_authority(
    runtime_config: &RuntimeConfig,
    state_store: &RuntimeGatewayStateStore,
    admin_tokens: &[RuntimeGatewayAdminToken],
    postgres_repository: Option<&prodex_storage_postgres_runtime::PostgresRepository>,
    async_runtime: &Arc<tokio::runtime::Runtime>,
    provider: &RuntimeLocalRewriteProviderOptions,
    provider_credential: Option<&RuntimeProjectedProviderCredential>,
) -> Result<RuntimeGatewayGovernanceAuthorityState> {
    let enforcing = runtime_config.governance.mode.is_enforcing();
    let deployment_mode = runtime_config.governance.mode;
    let policy_bootstrap = crate::runtime_governance::compile_runtime_governance_settings(
        &runtime_config.governance_policy,
    )?;
    let mut policy_snapshots =
        crate::runtime_governance::RuntimeGovernanceAuthoritySnapshotSet::bootstrap(
            policy_bootstrap,
            !enforcing,
        );
    let mut classification_snapshots =
        super::local_rewrite_classification_rules::RuntimeClassificationRulesSnapshotSet::bootstrap(
            &runtime_config.governance_policy,
            !enforcing,
        )?;
    let provider_bootstrap = super::local_rewrite_provider_registry::runtime_gateway_bootstrap_provider_registry_snapshot(
        &runtime_config.governance_policy,
        provider,
        provider_credential,
    )?;
    let mut provider_snapshots =
        super::local_rewrite_provider_registry::RuntimeGatewayProviderRegistrySnapshotSet::bootstrap(
            provider_bootstrap,
            !enforcing,
        );
    let routing_bootstrap =
        super::local_rewrite_provider_registry::runtime_gateway_bootstrap_routing_scores_snapshot(
            &runtime_config.governance_policy,
        );
    let mut routing_snapshots =
        super::local_rewrite_provider_registry::RuntimeGatewayRoutingScoresSnapshotSet::bootstrap(
            routing_bootstrap,
            !enforcing,
        );

    let wrap = |policy_snapshots,
                classification_snapshots,
                provider_snapshots,
                routing_snapshots,
                authority| {
        (
            Arc::new(ArcSwap::from_pointee(policy_snapshots)),
            Arc::new(ArcSwap::from_pointee(classification_snapshots)),
            Arc::new(ArcSwap::from_pointee(provider_snapshots)),
            Arc::new(ArcSwap::from_pointee(routing_snapshots)),
            authority,
        )
    };
    if !matches!(
        state_store,
        RuntimeGatewayStateStore::Sqlite { .. } | RuntimeGatewayStateStore::Postgres { .. }
    ) {
        if enforcing {
            anyhow::bail!("enforcing governance requires SQLite or PostgreSQL authority");
        }
        return Ok(wrap(
            policy_snapshots,
            classification_snapshots,
            provider_snapshots,
            routing_snapshots,
            None,
        ));
    }
    let mut tenants = runtime_config
        .governance_policy
        .authority_tenants
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for value in admin_tokens
        .iter()
        .filter_map(|token| token.tenant_id.as_deref())
    {
        let tenant = value
            .parse::<prodex_domain::TenantId>()
            .context("gateway admin tenant is invalid for governance authority")?;
        tenants.insert(tenant);
    }
    let sqlite_repository = match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => Some(
            prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path)
                .map_err(|_| anyhow::anyhow!("failed to open authoritative governance store"))?,
        ),
        _ => None,
    };
    let discovery_limit =
        (crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS + 1) as u16;
    let discovered = match state_store {
        RuntimeGatewayStateStore::Sqlite { .. } => sqlite_repository
            .as_ref()
            .expect("SQLite governance repository must be initialized")
            .governance_list_tenant_ids(discovery_limit),
        RuntimeGatewayStateStore::Postgres { .. } => async_runtime.block_on(
            postgres_repository
                .context("authoritative PostgreSQL governance repository is unavailable")?
                .governance_list_tenant_ids(discovery_limit),
        ),
        _ => unreachable!(),
    }
    .map_err(|_| anyhow::anyhow!("failed to discover authoritative governance tenants"))?;
    tenants.extend(discovered);
    if tenants.len() > crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS {
        anyhow::bail!("governance authority tenant limit exceeded");
    }
    if tenants.is_empty() && enforcing {
        anyhow::bail!("enforcing governance requires configured authority tenants");
    }
    let tenant_ids = Arc::new(Mutex::new(tenants));
    let authority = match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => RuntimeGovernanceAuthority::Sqlite {
            path: path.clone(),
            tenant_ids: Arc::clone(&tenant_ids),
        },
        RuntimeGatewayStateStore::Postgres { .. } => RuntimeGovernanceAuthority::Postgres {
            repository: postgres_repository
                .context("authoritative PostgreSQL governance repository is unavailable")?
                .clone(),
            runtime: Arc::clone(async_runtime),
            tenant_ids,
        },
        _ => unreachable!(),
    };
    let tenants = authority
        .tenant_ids()
        .map_err(|_| anyhow::anyhow!("failed to read authoritative governance tenants"))?;
    for tenant_id in &tenants {
        let policy = runtime_gateway_load_governance_snapshot(
            &authority,
            sqlite_repository.as_ref(),
            *tenant_id,
            prodex_storage::GovernanceArtifactKind::Policy,
            |artifact| {
                crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                    artifact,
                    deployment_mode,
                )
                .is_ok()
            },
        )
        .and_then(|stored| {
            let snapshot =
                crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                    &stored.compiled_artifact,
                    deployment_mode,
                )?;
            anyhow::ensure!(
                snapshot.application.policy.revision().to_string() == stored.revision_id,
                "policy artifact revision does not match stored revision"
            );
            Ok(snapshot)
        });
        let classification = runtime_gateway_load_governance_snapshot(
            &authority,
            sqlite_repository.as_ref(),
            *tenant_id,
            prodex_storage::GovernanceArtifactKind::ClassificationRules,
            |artifact| {
                super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
                    *tenant_id,
                    artifact,
                )
                .is_ok()
            },
        )
        .and_then(|stored| {
            let snapshot = super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
                *tenant_id,
                &stored.compiled_artifact,
            )?;
            anyhow::ensure!(
                snapshot.classification_rules().revision().as_str() == stored.revision_id,
                "classification artifact revision does not match stored revision"
            );
            Ok(snapshot)
        });
        let registry = runtime_gateway_load_governance_snapshot(
            &authority,
            sqlite_repository.as_ref(),
            *tenant_id,
            prodex_storage::GovernanceArtifactKind::ProviderRegistry,
            |artifact| {
                super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
                    artifact,
                    provider,
                    provider_credential,
                    deployment_mode,
                )
                .is_ok()
            },
        )
        .and_then(|stored| {
            let snapshot = super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
                &stored.compiled_artifact,
                provider,
                provider_credential,
                deployment_mode,
            )?;
            anyhow::ensure!(
                snapshot.revision().to_string() == stored.revision_id,
                "provider registry artifact revision does not match stored revision"
            );
            Ok(snapshot)
        });
        let routing = runtime_gateway_load_governance_snapshot(
            &authority,
            sqlite_repository.as_ref(),
            *tenant_id,
            prodex_storage::GovernanceArtifactKind::RoutingScores,
            |artifact| {
                super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(
                    artifact,
                )
                .is_ok()
            },
        )
        .and_then(|stored| {
            let snapshot = super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(
                &stored.compiled_artifact,
            )?;
            anyhow::ensure!(
                snapshot.revision.to_string() == stored.revision_id,
                "routing scores artifact revision does not match stored revision"
            );
            Ok(snapshot)
        });

        if enforcing {
            let unavailable = || {
                anyhow::anyhow!(
                    "authoritative governance store has no valid active or last-known-good snapshot"
                )
            };
            policy_snapshots = policy_snapshots
                .with_tenant_snapshot(*tenant_id, policy.map_err(|_| unavailable())?)?;
            classification_snapshots = classification_snapshots
                .with_tenant_snapshot(*tenant_id, classification.map_err(|_| unavailable())?)?;
            provider_snapshots = provider_snapshots
                .with_tenant_snapshot(*tenant_id, registry.map_err(|_| unavailable())?)?;
            routing_snapshots = routing_snapshots
                .with_tenant_snapshot(*tenant_id, routing.map_err(|_| unavailable())?)?;
        } else {
            if let Ok(policy) = policy {
                policy_snapshots = policy_snapshots.with_tenant_snapshot(*tenant_id, policy)?;
            }
            if let Ok(classification) = classification {
                classification_snapshots =
                    classification_snapshots.with_tenant_snapshot(*tenant_id, classification)?;
            }
            if let Ok(registry) = registry {
                provider_snapshots =
                    provider_snapshots.with_tenant_snapshot(*tenant_id, registry)?;
            }
            if let Ok(routing) = routing {
                routing_snapshots = routing_snapshots.with_tenant_snapshot(*tenant_id, routing)?;
            }
        }
    }
    Ok(wrap(
        policy_snapshots,
        classification_snapshots,
        provider_snapshots,
        routing_snapshots,
        Some(authority),
    ))
}

fn runtime_gateway_load_governance_snapshot(
    authority: &RuntimeGovernanceAuthority,
    sqlite_repository: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    tenant_id: prodex_domain::TenantId,
    kind: prodex_storage::GovernanceArtifactKind,
    validate_artifact: impl FnMut(&[u8]) -> bool,
) -> Result<prodex_storage::GovernanceSnapshot> {
    match authority {
        RuntimeGovernanceAuthority::Sqlite { .. } => sqlite_repository
            .context("authoritative SQLite governance repository is unavailable")?
            .load_snapshot(tenant_id, kind, validate_artifact)
            .map_err(anyhow::Error::from),
        RuntimeGovernanceAuthority::Postgres {
            repository,
            runtime,
            ..
        } => runtime
            .block_on(repository.governance_load_snapshot(tenant_id, kind, validate_artifact))
            .map_err(anyhow::Error::from),
    }
}

pub(super) struct RuntimeLocalRewriteWorkers {
    pub(super) worker_threads: Vec<thread::JoinHandle<()>>,
    pub(super) gemini_live_sidecar_addr: Option<std::net::SocketAddr>,
}

fn runtime_local_rewrite_usage_state(
    usage: BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
    path: PathBuf,
) -> RuntimeGatewayVirtualKeyUsageState {
    RuntimeGatewayVirtualKeyUsageState {
        usage: Arc::new(Mutex::new(usage)),
        path: Some(path),
        save_in_flight: Arc::new(AtomicBool::new(false)),
        save_dirty: Arc::new(AtomicBool::new(false)),
        usage_slots: Arc::new(tokio::sync::Semaphore::new(
            super::local_rewrite_gateway_usage::RUNTIME_GATEWAY_PENDING_USAGE_DELTA_LIMIT,
        )),
        pending_deltas: Arc::new(Mutex::new(Vec::new())),
        reconciliation: RuntimeGatewayReconciliationQueue::new(),
        request_ids: Arc::new(Mutex::new(BTreeSet::new())),
        typed_request_ids: Arc::new(Mutex::new(BTreeMap::new())),
        call_ids: Arc::new(Mutex::new(BTreeMap::new())),
        ledger_scopes: Arc::new(Mutex::new(BTreeMap::new())),
        durable_reservations: Arc::new(Mutex::new(BTreeMap::new())),
    }
}

fn runtime_local_rewrite_log_path(runtime_config: &RuntimeConfig) -> PathBuf {
    let log_path = initialize_runtime_proxy_log_path_from_config(runtime_config);
    for key in runtime_config.compatibility_defaults() {
        runtime_proxy_log_to_path(
            &log_path,
            &runtime_proxy_structured_log_message(
                "runtime_config_compatibility_default",
                [runtime_proxy_log_field("key", *key)],
            ),
        );
    }
    log_path
}

fn runtime_local_rewrite_server(
    preferred_listen_addr: Option<&str>,
) -> Result<(Arc<TinyServer>, std::net::SocketAddr)> {
    let bind_addr = preferred_listen_addr.unwrap_or("127.0.0.1:0");
    let server = Arc::new(TinyServer::http(bind_addr).map_err(|err| {
        anyhow::anyhow!("failed to bind runtime local rewrite proxy on {bind_addr}: {err}")
    })?);
    let listen_addr = server
        .server_addr()
        .to_ip()
        .context("runtime local rewrite proxy did not expose a TCP listen address")?;
    Ok((server, listen_addr))
}

pub(super) fn spawn_runtime_local_rewrite_workers(
    shared: &RuntimeLocalRewriteProxyShared,
    server: Option<&Arc<TinyServer>>,
    shutdown: &Arc<AtomicBool>,
    worker_count: usize,
    secret_refresh: Option<RuntimeGatewayCredentialRefreshPlan>,
    spawn_gemini_sidecar_listener: bool,
) -> Result<RuntimeLocalRewriteWorkers> {
    let mut worker_threads = Vec::new();
    if let Some(worker) = spawn_runtime_gateway_reservation_recovery_worker(shared, shutdown)? {
        worker_threads.push(worker);
    }
    if let Some(authority) = shared.governance_authority.clone() {
        worker_threads.push(
            shared
                .governance_sessions
                .spawn_durable_bank(
                    authority.clone(),
                    shared.runtime_shared.runtime_config.governance.mode
                        == prodex_config::GovernanceMode::BankEnforce,
                    Arc::clone(shutdown),
                )
                .map_err(|_| anyhow::anyhow!("failed to start governance session bank"))?,
        );
        worker_threads.push(
            shared
                .governance_audit_writer
                .spawn(authority, Arc::clone(shutdown))
                .map_err(|_| anyhow::anyhow!("failed to start governance audit writer"))?,
        );
    }
    if let Some(authority) = shared.governance_authority.clone() {
        let policy_snapshots = Arc::clone(&shared.governance_snapshot);
        let classification_snapshots = Arc::clone(&shared.classification_rules);
        let provider_snapshots = Arc::clone(&shared.governed_provider_registry);
        let routing_snapshots = Arc::clone(&shared.governed_routing_scores);
        let provider = shared.provider.clone();
        let provider_credential = shared.provider_credential.clone();
        let shutdown = Arc::clone(shutdown);
        let log_path = shared.runtime_shared.log_path.clone();
        let deployment_mode = shared.runtime_shared.runtime_config.governance.mode;
        worker_threads.push(thread::spawn(move || {
            let sqlite_repository = match &authority {
                RuntimeGovernanceAuthority::Sqlite { path, .. } => {
                    prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path).ok()
                }
                RuntimeGovernanceAuthority::Postgres { .. } => None,
            };
            while !shutdown.load(Ordering::SeqCst) {
                let discovered = match &authority {
                    RuntimeGovernanceAuthority::Sqlite { .. } => sqlite_repository
                        .as_ref()
                        .ok_or(prodex_storage::GovernanceRepositoryError::Database)
                        .and_then(|repository| {
                            repository.governance_list_tenant_ids(
                                (crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS
                                    + 1) as u16,
                            )
                        }),
                    RuntimeGovernanceAuthority::Postgres {
                        repository,
                        runtime,
                        ..
                    } => runtime.block_on(repository.governance_list_tenant_ids(
                        (crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS + 1)
                            as u16,
                    )),
                };
                let tenant_ids = discovered
                    .and_then(|discovered| {
                        authority.merge_tenant_ids(discovered)?;
                        authority.tenant_ids()
                    })
                    .unwrap_or_default();
                let mut next_policy = (*policy_snapshots.load_full()).clone();
                let mut next_classification = (*classification_snapshots.load_full()).clone();
                let mut next_provider = (*provider_snapshots.load_full()).clone();
                let mut next_routing = (*routing_snapshots.load_full()).clone();
                let mut policy_refreshed = 0usize;
                let mut classification_refreshed = 0usize;
                let mut provider_refreshed = 0usize;
                let mut routing_refreshed = 0usize;
                for tenant_id in &tenant_ids {
                    if let Ok(stored) = runtime_gateway_load_governance_snapshot(
                        &authority,
                        sqlite_repository.as_ref(),
                        *tenant_id,
                        prodex_storage::GovernanceArtifactKind::Policy,
                        |artifact| {
                            crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                                artifact,
                                deployment_mode,
                            )
                            .is_ok()
                        },
                    ) && let Ok(snapshot) =
                        crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                            &stored.compiled_artifact,
                            deployment_mode,
                        )
                        && snapshot.application.policy.revision().to_string()
                            == stored.revision_id
                        && let Ok(updated) =
                            next_policy.with_tenant_snapshot(*tenant_id, snapshot)
                    {
                        next_policy = updated;
                        policy_refreshed += 1;
                    }
                    if let Ok(stored) = runtime_gateway_load_governance_snapshot(
                        &authority,
                        sqlite_repository.as_ref(),
                        *tenant_id,
                        prodex_storage::GovernanceArtifactKind::ClassificationRules,
                        |artifact| {
                            super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
                                *tenant_id,
                                artifact,
                            )
                            .is_ok()
                        },
                    ) && let Ok(snapshot) = super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
                        *tenant_id,
                        &stored.compiled_artifact,
                    ) && snapshot.classification_rules().revision().as_str() == stored.revision_id
                        && let Ok(updated) = next_classification.with_tenant_snapshot(*tenant_id, snapshot)
                    {
                        next_classification = updated;
                        classification_refreshed += 1;
                    }
                    if let Ok(stored) = runtime_gateway_load_governance_snapshot(
                        &authority,
                        sqlite_repository.as_ref(),
                        *tenant_id,
                        prodex_storage::GovernanceArtifactKind::ProviderRegistry,
                        |artifact| {
                            super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
                                artifact,
                                &provider,
                                provider_credential.as_ref(),
                                deployment_mode,
                            )
                            .is_ok()
                        },
                    ) && let Ok(snapshot) = super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
                        &stored.compiled_artifact,
                        &provider,
                        provider_credential.as_ref(),
                        deployment_mode,
                    ) && snapshot.revision().to_string() == stored.revision_id
                        && let Ok(updated) = next_provider.with_tenant_snapshot(*tenant_id, snapshot)
                    {
                        next_provider = updated;
                        provider_refreshed += 1;
                    }
                    if let Ok(stored) = runtime_gateway_load_governance_snapshot(
                        &authority,
                        sqlite_repository.as_ref(),
                        *tenant_id,
                        prodex_storage::GovernanceArtifactKind::RoutingScores,
                        |artifact| {
                            super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(artifact)
                                .is_ok()
                        },
                    ) && let Ok(snapshot) = super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(
                        &stored.compiled_artifact,
                    ) && snapshot.revision.to_string() == stored.revision_id
                        && let Ok(updated) = next_routing.with_tenant_snapshot(*tenant_id, snapshot)
                    {
                        next_routing = updated;
                        routing_refreshed += 1;
                    }
                }
                if policy_refreshed > 0 {
                    policy_snapshots.store(Arc::new(next_policy));
                }
                if classification_refreshed > 0 {
                    classification_snapshots.store(Arc::new(next_classification));
                }
                if provider_refreshed > 0 {
                    provider_snapshots.store(Arc::new(next_provider));
                }
                if routing_refreshed > 0 {
                    routing_snapshots.store(Arc::new(next_routing));
                }
                if policy_refreshed + classification_refreshed + provider_refreshed + routing_refreshed > 0 {
                    runtime_proxy_log_to_path(
                        &log_path,
                        &format!(
                            "governance_snapshot_refresh status=success policy={policy_refreshed} classification_rules={classification_refreshed} provider_registry={provider_refreshed} routing_scores={routing_refreshed} configured={}",
                            tenant_ids.len()
                        ),
                    );
                } else {
                    runtime_proxy_log_to_path(
                        &log_path,
                        "governance_snapshot_refresh status=error action=retain_lkg",
                    );
                }
                for _ in 0..50 {
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }));
    }
    if let Some(siem_worker) = shared.gateway_observability.siem_worker.clone() {
        match &shared.gateway_state_store {
            RuntimeGatewayStateStore::Postgres { .. } => {
                let repository = shared
                    .gateway_postgres_repository
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("failed to open the SIEM governance outbox"))?;
                let governance_authority = shared.governance_authority.clone();
                let runtime = shared.runtime_shared.async_runtime.handle().clone();
                let shutdown = Arc::clone(shutdown);
                let log_path = shared.runtime_shared.log_path.clone();
                worker_threads.push(thread::spawn(move || {
                    while !shutdown.load(Ordering::SeqCst) {
                        let now_unix_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis()
                            .try_into()
                            .unwrap_or(u64::MAX);
                        let tenant_ids = governance_authority
                            .as_ref()
                            .and_then(|authority| authority.tenant_ids().ok())
                            .unwrap_or_default();
                        let status = if siem_worker
                            .run_once_postgres(&repository, &runtime, &tenant_ids, now_unix_ms)
                            .is_ok()
                        {
                            "success"
                        } else {
                            "error"
                        };
                        runtime_proxy_log_to_path(
                            &log_path,
                            &format!("governance_siem_worker status={status} backend=postgres"),
                        );
                        for _ in 0..50 {
                            if shutdown.load(Ordering::SeqCst) {
                                break;
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                    }
                }));
            }
            RuntimeGatewayStateStore::Sqlite { path } => {
                let repository = prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(
                    path,
                )
                .map_err(|_| anyhow::anyhow!("failed to open the SIEM governance outbox"))?;
                let shutdown = Arc::clone(shutdown);
                let log_path = shared.runtime_shared.log_path.clone();
                worker_threads.push(thread::spawn(move || {
                    while !shutdown.load(Ordering::SeqCst) {
                        let now_unix_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis()
                            .try_into()
                            .unwrap_or(u64::MAX);
                        match siem_worker.run_once(&repository, now_unix_ms) {
                    Ok(report) => match repository
                        .aggregate_outbox_health()
                        .and_then(|health| {
                            siem_worker
                                .plan_health(health, now_unix_ms)
                                .map_err(|_| {
                                    prodex_storage_sqlite_runtime::GovernanceRepositoryError::Database
                                })
                        }) {
                        Ok(metric) => {
                            let status = metric
                                .status_label
                                .as_metric_label()
                                .map(|(_, value)| value)
                                .unwrap_or("error");
                            runtime_proxy_log_to_path(
                                &log_path,
                                &format!(
                                    "governance_siem_worker status=success selected={} delivered={} retried={} dead_lettered={} {}={} {}={} {}={} health={status}",
                                    report.selected,
                                    report.delivered,
                                    report.retried,
                                    report.dead_lettered,
                                    metric.pending_metric_name,
                                    metric.pending,
                                    metric.dead_letter_metric_name,
                                    metric.dead_lettered,
                                    metric.lag_metric_name,
                                    metric.lag_milliseconds,
                                ),
                            );
                        }
                        Err(_) => runtime_proxy_log_to_path(
                            &log_path,
                            "governance_siem_worker status=error code=health_unavailable",
                        ),
                    },
                    Err(_) => runtime_proxy_log_to_path(
                        &log_path,
                        "governance_siem_worker status=error code=outbox_unavailable",
                    ),
                }
                        for _ in 0..50 {
                            if shutdown.load(Ordering::SeqCst) {
                                break;
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                    }
                }));
            }
            RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
                anyhow::bail!("configured SIEM worker requires a durable governance outbox");
            }
        }
    }
    if let Some(pool) = shared.gemini_oauth_pool.as_ref()
        && let Some(worker) = pool.spawn_quota_refresh(shared.runtime_shared.log_path.clone())
    {
        worker_threads.push(worker);
    }
    if shared.gateway_sso.oidc.is_some() || shared.gateway_sso.workload_identity.is_some() {
        let shared = shared.clone();
        let shutdown = Arc::clone(shutdown);
        worker_threads.push(thread::spawn(move || {
            runtime_gateway_run_oidc_background_refresh_loop(shared, shutdown);
        }));
    }
    if let Some(secret_refresh) = secret_refresh {
        worker_threads.push(runtime_gateway_spawn_secret_refresh(
            shared.clone(),
            Arc::clone(shutdown),
            secret_refresh,
        ));
    }
    let gemini_live_sidecar_addr = if spawn_gemini_sidecar_listener
        && shared
            .runtime_shared
            .runtime_config
            .governance
            .mode
            .allows_anonymous_compatibility()
        && matches!(
            &shared.provider,
            RuntimeLocalRewriteProviderOptions::Gemini { .. }
        ) {
        Some(spawn_runtime_gemini_live_sidecar(
            shared.clone(),
            Arc::clone(shutdown),
            &mut worker_threads,
        )?)
    } else {
        None
    };
    for worker_index in 0..worker_count {
        let Some(server) = server else {
            break;
        };
        let worker = spawn_runtime_local_rewrite_listener_worker(
            worker_index,
            Arc::clone(server),
            Arc::clone(shutdown),
            shared.clone(),
        );
        match worker {
            Ok(worker) => worker_threads.push(worker),
            Err(err) => {
                shutdown.store(true, Ordering::SeqCst);
                for _ in 0..worker_index {
                    server.unblock();
                }
                return Err(err.into());
            }
        }
    }
    Ok(RuntimeLocalRewriteWorkers {
        worker_threads,
        gemini_live_sidecar_addr,
    })
}

fn build_runtime_local_rewrite_http_client(
    runtime_config: &RuntimeConfig,
) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_millis(
            runtime_config.tuning.http_connect_timeout_ms,
        ))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .context("failed to build runtime local rewrite HTTP client")
}

fn handle_runtime_local_rewrite_proxy_request(
    request: tiny_http::Request,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let target = match prodex_gateway_http::CanonicalRequestTarget::parse(request.url()) {
        Ok(target) => target,
        Err(_) => {
            let _ = request.respond(build_runtime_proxy_json_error_response(
                400,
                "invalid_request_target",
                "request target is invalid",
            ));
            return;
        }
    };
    run_runtime_local_rewrite_pipeline(RuntimeLocalRewriteRequest::tiny(request), target, shared);
}
#[cfg(test)]
mod request_guard_tests {
    use super::super::local_rewrite_gateway_usage::RuntimeGatewayUsageRequestGuard;
    use super::{RuntimeGovernanceAuthority, runtime_gateway_try_reserve_background_task};
    use prodex_domain::TenantId;
    use prodex_storage::GovernanceRepositoryError;
    use std::cell::Cell;
    use std::collections::BTreeSet;
    use std::sync::{Arc, Mutex};

    #[test]
    fn gateway_usage_request_guard_releases_request_id() {
        let request_ids = Arc::new(Mutex::new(BTreeSet::from([7])));
        {
            let _guard = RuntimeGatewayUsageRequestGuard {
                request_ids: Arc::clone(&request_ids),
                reconciliation: super::RuntimeGatewayReconciliationQueue::new(),
                request_id: 7,
            };
        }

        assert!(request_ids.lock().unwrap().is_empty());
    }

    #[test]
    fn gateway_background_task_slots_are_bounded() {
        let slots = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = runtime_gateway_try_reserve_background_task(&slots).unwrap();
        assert!(runtime_gateway_try_reserve_background_task(&slots).is_none());
        drop(permit);
        assert!(runtime_gateway_try_reserve_background_task(&slots).is_some());
    }

    #[test]
    fn governance_tenant_capacity_is_reserved_before_commit() {
        let configured = (0..crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS)
            .map(|_| TenantId::new())
            .collect();
        let authority = RuntimeGovernanceAuthority::Sqlite {
            path: "unused.sqlite".into(),
            tenant_ids: Arc::new(Mutex::new(configured)),
        };
        let committed = Cell::new(false);

        assert_eq!(
            authority.commit_for_tenant(TenantId::new(), || {
                committed.set(true);
                Ok(())
            }),
            Err(GovernanceRepositoryError::SnapshotUnavailable)
        );
        assert!(!committed.get());
    }
}
