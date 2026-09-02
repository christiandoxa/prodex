use super::super::copilot_instructions::runtime_copilot_init_current_workspace_custom_instructions;
use super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
mod background_workers;
mod context;
mod governance_authority;
mod governance_bundle;
mod governance_invalidation;
mod governance_publication;
mod governance_refresh;
mod listener_worker;
pub(super) use self::background_workers::{
    RuntimeLocalRewriteWorkers, spawn_runtime_local_rewrite_workers,
};
use self::background_workers::{
    runtime_local_rewrite_log_path, runtime_local_rewrite_server, runtime_local_rewrite_usage_state,
};
pub(super) use self::context::{
    RuntimeLocalRewriteProcessServices, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteRequestContext,
};
pub(super) use self::governance_authority::{
    runtime_gateway_governance_authority, runtime_gateway_load_governance_snapshot,
};
use self::governance_bundle::RuntimeGovernanceSnapshotBundleSet;
use self::governance_invalidation::spawn_runtime_gateway_governance_invalidation_worker;
use self::governance_refresh::spawn_runtime_gateway_governance_refresh_worker;
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
    RuntimeLocalRewriteContinuationReader, RuntimeLocalRewriteLiveResponse,
    RuntimeLocalRewriteSsePrefetch, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult,
};
use super::provider_bridge::runtime_provider_label;
use crate::app_commands::runtime_launch::gateway_config::gateway_siem_export::RuntimeSiemWorkerConfig;
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
use std::io::{self, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
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

pub(super) struct RuntimeLocalRewriteAsyncResponse {
    pub(super) response: Option<reqwest::Response>,
    status: reqwest::StatusCode,
    headers: reqwest::header::HeaderMap,
    pub(super) async_runtime: Arc<tokio::runtime::Runtime>,
    pub(super) stream_idle_timeout_ms: u64,
    pub(super) pending: Vec<u8>,
    pub(super) reader: Option<RuntimeLocalRewriteContinuationReader>,
}

impl RuntimeLocalRewriteAsyncResponse {
    pub(super) fn new(
        response: reqwest::Response,
        async_runtime: Arc<tokio::runtime::Runtime>,
        stream_idle_timeout_ms: u64,
    ) -> Self {
        let status = response.status();
        let headers = response.headers().clone();
        Self {
            response: Some(response),
            status,
            headers,
            async_runtime,
            stream_idle_timeout_ms,
            pending: Vec::new(),
            reader: None,
        }
    }

    pub(super) fn status(&self) -> reqwest::StatusCode {
        self.status
    }

    pub(super) fn headers(&self) -> &reqwest::header::HeaderMap {
        &self.headers
    }

    pub(super) fn into_reader(mut self) -> Box<dyn Read + Send> {
        if let Some(reader) = self.reader.take() {
            return Box::new(reader);
        }
        Box::new(RuntimeLocalRewriteSsePrefetch::spawn(self, None).into_reader())
    }
}

impl Read for RuntimeLocalRewriteAsyncResponse {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.reader.is_none() {
            let response = self.response.take().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "runtime upstream stream reader was already handed off",
                )
            })?;
            let prefetch = RuntimeLocalRewriteSsePrefetch::spawn_parts(
                response,
                Arc::clone(&self.async_runtime),
                self.stream_idle_timeout_ms,
                std::mem::take(&mut self.pending),
                None,
            );
            self.reader = Some(prefetch.into_reader());
        }
        self.reader
            .as_mut()
            .expect("runtime local rewrite stream reader should be present")
            .read(buffer)
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeGovernanceArtifactRefreshOutcome {
    Published,
    Invalidated,
}

impl RuntimeGovernanceAuthority {
    pub(super) fn tenant_ids(
        &self,
    ) -> std::result::Result<Vec<prodex_domain::TenantId>, prodex_storage::GovernanceRepositoryError>
    {
        let tenant_ids = match self {
            Self::Sqlite { tenant_ids, .. } | Self::Postgres { tenant_ids, .. } => tenant_ids,
        };
        let tenant_ids = tenant_ids.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(tenant_ids.iter().copied().collect())
    }

    pub(super) fn commit_for_tenant<T>(
        &self,
        tenant_id: prodex_domain::TenantId,
        operation: impl FnOnce() -> std::result::Result<T, prodex_storage::GovernanceRepositoryError>,
    ) -> std::result::Result<T, prodex_storage::GovernanceRepositoryError> {
        let tenant_ids = match self {
            Self::Sqlite { tenant_ids, .. } | Self::Postgres { tenant_ids, .. } => tenant_ids,
        };
        let mut tenant_ids = tenant_ids.lock().unwrap_or_else(PoisonError::into_inner);
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
        let mut tenant_ids = tenant_ids.lock().unwrap_or_else(PoisonError::into_inner);
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
    #[cfg(test)]
    let (listener_ready_tx, listener_ready_rx) = std::sync::mpsc::channel();
    let RuntimeLocalRewriteWorkers {
        worker_threads,
        gemini_live_sidecar_addr,
    } = spawn_runtime_local_rewrite_workers(
        &shared,
        Some(&server),
        &shutdown,
        worker_count,
        secret_refresh,
        #[cfg(test)]
        Some(listener_ready_tx),
        true,
    )?;
    #[cfg(test)]
    for _ in 0..worker_count {
        listener_ready_rx
            .recv()
            .expect("runtime local rewrite listener should start");
    }
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
    let log_path = runtime_local_rewrite_log_path(&runtime_config)?;
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
        smart_context_engine: std::sync::Arc::new(crate::RuntimeSmartContextEngine::default()),
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
    let (governance_snapshots, governance_authority) = runtime_gateway_governance_authority(
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
        &runtime_shared,
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
    let copilot_oauth_pool =
        runtime_copilot_oauth_pool_from_provider(&provider, Arc::clone(&runtime_shared.runtime));
    if matches!(
        &provider,
        RuntimeLocalRewriteProviderOptions::Copilot { .. }
    ) {
        runtime_copilot_init_current_workspace_custom_instructions();
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    let gateway_virtual_keys = Arc::new(Mutex::new(gateway_virtual_key_entries));
    let gateway_credentials = RuntimeGatewayCredentialState::new(
        runtime_gateway_initial_credential_snapshot(
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
        ),
        secret_refresh.is_some(),
    );
    let initial_gateway_credentials = gateway_credentials.current.load_full();
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
        governance_snapshots,
        governance_authority,
        governance_refresh_requested: Arc::new(AtomicBool::new(false)),
        api_key_cursor: Arc::new(AtomicUsize::new(0)),
        client: build_runtime_local_rewrite_http_client(&runtime_config)?,
        // ponytail: one bounded async pump per inspectable SSE response; saturation passes through.
        provider_sse_prefetch_slots: Arc::new(tokio::sync::Semaphore::new(
            active_request_limit.max(1),
        )),
        gateway_oidc_http_cache: Arc::new(Mutex::new(BTreeMap::new())),
        gateway_oidc_jwks_snapshot: Arc::new(arc_swap::ArcSwapOption::empty()),
        gateway_workload_jwks_snapshot: Arc::new(arc_swap::ArcSwapOption::empty()),
        gateway_browser: Default::default(),
        gateway_credentials,
        gateway_postgres_repository,
        gateway_redis_rate_limit_executor,
        gateway_policy_version: prodex_runtime_policy::runtime_policy_summary()
            .ok()
            .flatten()
            .map(|summary| summary.version),
        gateway_virtual_key_store_path: gateway_state_store.key_store_path().to_path_buf(),
        gateway_usage: runtime_local_rewrite_usage_state(
            gateway_virtual_key_usage,
            gateway_state_store.usage_path().to_path_buf(),
        ),
        gateway_state_store,
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
        governance: process.governance_snapshots.load_full(),
        process,
        upstream_base_url,
        provider: Arc::clone(&initial_gateway_credentials.provider),
        provider_credential: initial_gateway_credentials.provider_credential.clone(),
        governed_pricing: None,
        gateway_auth_token_hash: initial_gateway_credentials.auth_token_hash.clone(),
        gateway_admin_tokens: initial_gateway_credentials.admin_tokens.clone(),
        gateway_sso: initial_gateway_credentials.sso.clone(),
        gateway_virtual_keys: Arc::clone(&initial_gateway_credentials.virtual_keys),
        gateway_guardrail_webhook: Arc::clone(&initial_gateway_credentials.guardrail_webhook),
        gateway_observability: Arc::clone(&initial_gateway_credentials.observability),
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
    let request = RuntimeLocalRewriteRequest::tiny(request);
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
    run_runtime_local_rewrite_pipeline(request, target, shared);
}
#[cfg(test)]
#[path = "local_rewrite_request_guard_tests.rs"]
mod request_guard_tests;
