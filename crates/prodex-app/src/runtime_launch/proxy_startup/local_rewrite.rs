use super::super::copilot_instructions::runtime_copilot_init_current_workspace_custom_instructions;
use super::deepseek_rewrite::*;
#[cfg(test)]
pub(crate) use super::local_rewrite_constraints::start_runtime_gateway_rewrite_proxy;
pub(crate) use super::local_rewrite_constraints::{
    start_runtime_gateway_rewrite_proxy_with_runtime_config, start_runtime_local_rewrite_proxy,
};
use super::local_rewrite_copilot::{
    RuntimeCopilotOAuthPool, runtime_copilot_oauth_pool_from_provider,
};
use super::local_rewrite_gateway_admin_auth::{
    RuntimeGatewayOidcJwksSnapshot, runtime_gateway_run_oidc_background_refresh_loop,
};
pub(crate) use super::local_rewrite_gateway_config::{
    RuntimeGatewayAdminRole, RuntimeGatewayAdminToken, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewayOidcConfig, RuntimeGatewaySsoConfig,
    RuntimeGatewayStateStore, runtime_gateway_postgres_repository,
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
    runtime_gateway_virtual_key_store_load, runtime_gateway_virtual_key_store_load_strict,
};
pub(super) use super::local_rewrite_gateway_ledger::{
    runtime_gateway_billing_ledger_load, schedule_runtime_gateway_billing_ledger_reconcile,
};
use super::local_rewrite_gateway_store_types::RuntimeGatewayVirtualKeyEntry;
#[cfg(test)]
pub(super) use super::local_rewrite_gateway_usage::runtime_gateway_virtual_key_usage_apply_deltas;
pub(super) use super::local_rewrite_gateway_usage::{
    runtime_gateway_virtual_key_usage_load_strict, schedule_runtime_gateway_virtual_key_usage_save,
};
pub(super) use super::local_rewrite_gateway_usage_backend::RuntimeGatewayVirtualKeyUsageDelta;
pub(super) use super::local_rewrite_gateway_util::runtime_gateway_generate_virtual_key_token;
use super::local_rewrite_gemini::{
    RuntimeGeminiOAuthPool, runtime_gemini_oauth_pool_from_provider,
};
use super::local_rewrite_gemini_live::spawn_runtime_gemini_live_sidecar;
pub(super) use super::local_rewrite_model_memory::{
    RuntimeLocalRewriteModelMemoryState, runtime_local_rewrite_model_selection,
};
pub(crate) use super::local_rewrite_options::{
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
    RuntimeProjectedProviderCredential,
};
use super::local_rewrite_pipeline::run_runtime_local_rewrite_pipeline;
#[cfg(test)]
pub(super) use super::local_rewrite_pipeline::runtime_local_rewrite_remote_compact_unsupported_message;
pub(super) use super::local_rewrite_upstream::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult,
};
use super::provider_bridge::runtime_provider_openai_contract;
use super::*;
use anyhow::Context;
use arc_swap::ArcSwapOption;
use std::collections::{BTreeMap, BTreeSet};

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
pub(super) struct RuntimeLocalRewriteProxyShared {
    pub(super) runtime_shared: RuntimeRotationProxyShared,
    pub(super) upstream_base_url: String,
    pub(super) mount_path: String,
    pub(super) provider: RuntimeLocalRewriteProviderOptions,
    pub(super) provider_credential: Option<RuntimeProjectedProviderCredential>,
    pub(super) deepseek_conversations: RuntimeDeepSeekConversationStore,
    pub(super) deepseek_pending_messages: RuntimeDeepSeekPendingMessages,
    pub(super) gemini_conversations: RuntimeDeepSeekConversationStore,
    pub(super) gemini_oauth_pool: Option<RuntimeGeminiOAuthPool>,
    pub(super) copilot_oauth_pool: Option<RuntimeCopilotOAuthPool>,
    pub(super) model_memory: RuntimeLocalRewriteModelMemory,
    pub(super) api_key_cursor: Arc<AtomicUsize>,
    pub(super) client: reqwest::blocking::Client,
    pub(super) gateway_oidc_http_cache:
        Arc<Mutex<BTreeMap<String, RuntimeGatewayOidcHttpCacheEntry>>>,
    pub(super) gateway_oidc_jwks_snapshot: Arc<ArcSwapOption<RuntimeGatewayOidcJwksSnapshot>>,
    pub(super) gateway_admin_idempotency_keys: Arc<Mutex<BTreeSet<String>>>,
    pub(super) gateway_credentials: RuntimeGatewayCredentialState,
    pub(super) gateway_auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(super) gateway_admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(super) gateway_sso: RuntimeGatewaySsoConfig,
    pub(super) gateway_state_store: RuntimeGatewayStateStore,
    pub(super) gateway_postgres_repository:
        Option<prodex_storage_postgres_runtime::PostgresRepository>,
    pub(super) gateway_redis_rate_limit_executor:
        Option<prodex_storage_redis_runtime::RedisRateLimitExecutor>,
    pub(super) gateway_policy_version: Option<u32>,
    pub(super) gateway_virtual_keys: Arc<Mutex<Vec<RuntimeGatewayVirtualKeyEntry>>>,
    pub(super) gateway_virtual_key_store_path: PathBuf,
    pub(super) gateway_usage: RuntimeGatewayVirtualKeyUsageState,
    pub(super) gateway_route_aliases: Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>,
    pub(super) gateway_request_constraints: prodex_provider_core::ProviderRequestConstraintPolicy,
    pub(super) gateway_route_load: RuntimeGatewayRouteLoadState,
    pub(super) gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(super) gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(super) gateway_call_id_header: Option<String>,
    pub(super) gateway_observability: RuntimeGatewayObservabilityConfig,
    pub(super) gateway_observability_slots: Arc<tokio::sync::Semaphore>,
    pub(super) allow_local_file_access: bool,
    pub(super) gateway_draining: Arc<AtomicBool>,
}

#[allow(dead_code)]
impl RuntimeLocalRewriteProxyShared {
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
    pub(super) pending_deltas: Arc<Mutex<Vec<RuntimeGatewayVirtualKeyUsageDelta>>>,
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

pub(super) fn start_runtime_local_rewrite_proxy_with_file_access(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
    runtime_config: Arc<RuntimeConfig>,
    allow_local_file_access: bool,
    secret_refresh: Option<RuntimeGatewayCredentialRefreshPlan>,
    gateway_request_constraints: prodex_provider_core::ProviderRequestConstraintPolicy,
) -> Result<RuntimeRotationProxy> {
    let RuntimeLocalRewriteProxyStartOptions {
        paths,
        state,
        upstream_base_url,
        provider,
        upstream_no_proxy,
        smart_context_enabled,
        presidio_redaction_enabled,
        model_context_window_tokens,
        preferred_listen_addr,
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
    let (provider, provider_credential) = provider.into_runtime_parts();
    let log_path = runtime_local_rewrite_log_path(&runtime_config);
    let (server, listen_addr) = runtime_local_rewrite_server(preferred_listen_addr)?;
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
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
    };
    let gateway_postgres_repository =
        runtime_gateway_postgres_repository(&gateway_state_store, worker_count)?;
    let gateway_redis_rate_limit_executor =
        runtime_gateway_redis_rate_limit_executor(&gateway_state_store, &runtime_shared)?;
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
            Some(runtime_presidio_redaction_config(paths)?)
        } else {
            None
        },
    );
    let bridge_kind = provider.bridge_kind();
    let openai_contract = runtime_provider_openai_contract(bridge_kind);
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
            "runtime local rewrite proxy started listen_addr={listen_addr} smart_context_enabled={smart_context_enabled} presidio_redaction_enabled={presidio_redaction_enabled} upstream_base_url={upstream_base_url} upstream_proxy_mode={} provider={} client_format={} upstream_format={} response_format={} endpoint={} auth_required={} virtual_keys={} route_aliases={} guardrail_blocked_keywords={} guardrail_blocked_output_keywords={} guardrail_allowed_models={} observability_sinks={}",
            runtime_upstream_proxy_mode_label(true),
            super::provider_bridge::runtime_provider_label(bridge_kind),
            openai_contract.client_request_format.label(),
            openai_contract.upstream_request_format.label(),
            openai_contract.response_format.label(),
            openai_contract.canonical_client_endpoint,
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
    let shared = RuntimeLocalRewriteProxyShared {
        runtime_shared: runtime_shared.clone(),
        upstream_base_url,
        mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        provider,
        provider_credential,
        deepseek_conversations: RuntimeDeepSeekConversationStore::default(),
        deepseek_pending_messages: Arc::new(Mutex::new(BTreeMap::new())),
        gemini_conversations: RuntimeDeepSeekConversationStore::default(),
        gemini_oauth_pool,
        copilot_oauth_pool,
        model_memory: Arc::new(Mutex::new(RuntimeLocalRewriteModelMemoryState::default())),
        api_key_cursor: Arc::new(AtomicUsize::new(0)),
        client: build_runtime_local_rewrite_http_client(&runtime_config)?,
        gateway_oidc_http_cache: Arc::new(Mutex::new(BTreeMap::new())),
        gateway_oidc_jwks_snapshot: Arc::new(ArcSwapOption::empty()),
        gateway_admin_idempotency_keys: Arc::new(Mutex::new(BTreeSet::new())),
        gateway_credentials,
        gateway_auth_token_hash,
        gateway_admin_tokens,
        gateway_sso,
        gateway_state_store,
        gateway_postgres_repository,
        gateway_redis_rate_limit_executor,
        gateway_policy_version: prodex_runtime_policy::runtime_policy_summary()
            .ok()
            .flatten()
            .map(|summary| summary.version),
        gateway_virtual_keys,
        gateway_virtual_key_store_path,
        gateway_usage: runtime_local_rewrite_usage_state(
            gateway_virtual_key_usage,
            gateway_virtual_key_usage_path,
        ),
        gateway_route_aliases,
        gateway_request_constraints,
        gateway_route_load: Arc::new(Mutex::new(BTreeMap::new())),
        gateway_guardrails,
        gateway_guardrail_webhook,
        gateway_call_id_header,
        gateway_observability,
        gateway_observability_slots: Arc::new(tokio::sync::Semaphore::new(
            RUNTIME_GATEWAY_BACKGROUND_TASK_LIMIT,
        )),
        allow_local_file_access,
        gateway_draining: Arc::clone(&shutdown),
    };
    let RuntimeLocalRewriteWorkers {
        worker_threads,
        gemini_live_sidecar_addr,
    } = spawn_runtime_local_rewrite_workers(
        &shared,
        &server,
        &shutdown,
        worker_count,
        secret_refresh,
    )?;

    Ok(RuntimeRotationProxy {
        runtime_config: Arc::clone(&runtime_config),
        server,
        shutdown,
        worker_threads,
        accept_worker_count: worker_count,
        listen_addr,
        gemini_live_sidecar_addr,
        gemini_live_sidecar_model: gemini_live_sidecar_addr.map(|_| {
            super::local_rewrite_gemini_live::runtime_gemini_live_default_model().to_string()
        }),
        log_path,
        active_request_count: Arc::clone(&runtime_shared.active_request_count),
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
    })
}

struct RuntimeLocalRewriteWorkers {
    worker_threads: Vec<thread::JoinHandle<()>>,
    gemini_live_sidecar_addr: Option<std::net::SocketAddr>,
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
        pending_deltas: Arc::new(Mutex::new(Vec::new())),
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

fn spawn_runtime_local_rewrite_workers(
    shared: &RuntimeLocalRewriteProxyShared,
    server: &Arc<TinyServer>,
    shutdown: &Arc<AtomicBool>,
    worker_count: usize,
    secret_refresh: Option<RuntimeGatewayCredentialRefreshPlan>,
) -> Result<RuntimeLocalRewriteWorkers> {
    if let Some(pool) = shared.gemini_oauth_pool.as_ref() {
        pool.spawn_quota_refresh(shared.runtime_shared.log_path.clone());
    }
    let mut worker_threads = Vec::new();
    if shared.gateway_sso.oidc.is_some() {
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
    let gemini_live_sidecar_addr = if matches!(
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
    for _ in 0..worker_count {
        let server = Arc::clone(server);
        let shutdown = Arc::clone(shutdown);
        let shared = shared.clone();
        worker_threads.push(thread::spawn(move || {
            runtime_local_rewrite_worker_loop(server, shutdown, shared)
        }));
    }
    Ok(RuntimeLocalRewriteWorkers {
        worker_threads,
        gemini_live_sidecar_addr,
    })
}

fn runtime_local_rewrite_worker_loop(
    server: Arc<TinyServer>,
    shutdown: Arc<AtomicBool>,
    shared: RuntimeLocalRewriteProxyShared,
) {
    loop {
        match server.recv() {
            Ok(request) => {
                let request_shared = runtime_gateway_pin_request_credentials(&shared);
                let result = crate::runtime_panic::catch_runtime_unwind_silently(|| {
                    handle_runtime_local_rewrite_proxy_request(request, &request_shared);
                });
                if let Err(panic) = result {
                    runtime_proxy_log(
                        &request_shared.runtime_shared,
                        format!(
                            "runtime_proxy_worker_panic lane=local_rewrite panic={}",
                            crate::runtime_panic::runtime_panic_payload_label(panic.as_ref())
                        ),
                    );
                }
            }
            Err(_) if shutdown.load(Ordering::SeqCst) => break,
            Err(_) => {}
        }
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
    }
}

fn build_runtime_local_rewrite_http_client(
    runtime_config: &RuntimeConfig,
) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_millis(
            runtime_config.tuning.http_connect_timeout_ms,
        ))
        .no_proxy()
        .build()
        .context("failed to build runtime local rewrite HTTP client")
}

fn handle_runtime_local_rewrite_proxy_request(
    request: tiny_http::Request,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    run_runtime_local_rewrite_pipeline(request, shared);
}
#[cfg(test)]
mod request_guard_tests {
    use super::super::local_rewrite_gateway_usage::RuntimeGatewayUsageRequestGuard;
    use super::*;

    #[test]
    fn gateway_usage_request_guard_releases_request_id() {
        let request_ids = Arc::new(Mutex::new(BTreeSet::from([7])));
        {
            let _guard = RuntimeGatewayUsageRequestGuard {
                request_ids: Arc::clone(&request_ids),
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
}
