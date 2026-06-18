use super::super::copilot_instructions::runtime_copilot_init_current_workspace_custom_instructions;
use super::deepseek_rewrite::*;
use super::local_rewrite_copilot::{
    RuntimeCopilotOAuthPool, runtime_copilot_oauth_pool_from_provider,
};
use super::local_rewrite_gateway_admin_router::{
    runtime_gateway_admin_response, runtime_gateway_request_path_is_admin,
};
use super::local_rewrite_gateway_budget::runtime_gateway_budget_group_rejection;
pub(crate) use super::local_rewrite_gateway_config::{
    RuntimeGatewayAdminRole, RuntimeGatewayAdminToken, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewayOidcConfig, RuntimeGatewaySsoConfig,
    RuntimeGatewayStateStore,
};
use super::local_rewrite_gateway_file_ledger::{
    runtime_gateway_file_ledger_append_deltas, runtime_gateway_file_ledger_load,
    runtime_gateway_file_ledger_reconcile_response,
};
pub(super) use super::local_rewrite_gateway_guardrail_webhook::runtime_gateway_guardrail_webhook_block;
use super::local_rewrite_gateway_key_store_backend::{
    runtime_gateway_postgres_load_key_store, runtime_gateway_redis_load_key_store,
    runtime_gateway_sqlite_load_key_store,
};
use super::local_rewrite_gateway_ledger_types::RuntimeGatewayBillingLedgerEntry;
use super::local_rewrite_gateway_redis_ledger::{
    runtime_gateway_redis_ledger_load, runtime_gateway_redis_ledger_reconcile_response,
};
use super::local_rewrite_gateway_route_load::RuntimeGatewayRouteLoadGuard;
use super::local_rewrite_gateway_sql_ledger::{
    runtime_gateway_postgres_ledger_load, runtime_gateway_postgres_ledger_reconcile_response,
    runtime_gateway_sqlite_ledger_load, runtime_gateway_sqlite_ledger_reconcile_response,
};
use super::local_rewrite_gateway_store_file::runtime_gateway_virtual_key_store_file_load;
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayVirtualKeyEntry, RuntimeGatewayVirtualKeySource,
    RuntimeGatewayVirtualKeyStoreFile, runtime_gateway_virtual_key_entry_from_stored,
};
pub(super) use super::local_rewrite_gateway_usage_backend::RuntimeGatewayVirtualKeyUsageDelta;
use super::local_rewrite_gateway_usage_backend::{
    runtime_gateway_postgres_usage_apply_deltas, runtime_gateway_postgres_usage_load,
    runtime_gateway_redis_usage_apply_deltas, runtime_gateway_redis_usage_load,
    runtime_gateway_sqlite_usage_apply_deltas, runtime_gateway_sqlite_usage_load,
};
pub(super) use super::local_rewrite_gateway_util::{
    runtime_gateway_generate_virtual_key_token, runtime_gateway_unix_epoch_seconds,
};
use super::local_rewrite_gemini::{
    RuntimeGeminiOAuthPool, runtime_gemini_oauth_pool_from_provider,
};
use super::local_rewrite_gemini_compact::respond_runtime_gemini_compact_request;
use super::local_rewrite_gemini_live::{
    handle_runtime_gemini_live_websocket_request, spawn_runtime_gemini_live_sidecar,
};
pub(super) use super::local_rewrite_model_memory::{
    RuntimeLocalRewriteModelMemoryState, runtime_local_rewrite_model_selection,
};
#[cfg(test)]
pub(super) use super::local_rewrite_model_memory::{
    runtime_local_rewrite_model_allows_session_memory, runtime_local_rewrite_model_scope,
};
pub(crate) use super::local_rewrite_options::{
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
};
use super::local_rewrite_response::{
    respond_runtime_local_rewrite_proxy_request, runtime_local_rewrite_response_with_call_id,
};
use super::local_rewrite_upstream::send_runtime_local_rewrite_upstream_request;
pub(super) use super::local_rewrite_upstream::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult,
};
use super::provider_bridge::{
    RuntimeProviderGatewaySpendEvent, runtime_provider_gateway_cost_for_request,
    runtime_provider_models_buffered_response, runtime_provider_openai_contract,
    runtime_provider_request_ledger_message,
};
use super::*;
use fs2::FileExt;
use prodex_provider_core::{calculate_cost_microusd, estimate_request_input_tokens};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;

pub(crate) const RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH: &str = "/v1";
pub(super) const RUNTIME_LOCAL_REWRITE_PROFILE: &str = "local";
pub(super) const RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY: &str = "prodex:gateway:virtual_keys";
pub(super) const RUNTIME_GATEWAY_REDIS_KEY_STORE_LOCK: &str = "prodex:gateway:virtual_keys:lock";
const RUNTIME_GATEWAY_REDIS_USAGE_KEY: &str = "prodex:gateway:virtual_key_usage";
const RUNTIME_GATEWAY_REDIS_USAGE_LOCK: &str = "prodex:gateway:virtual_key_usage:lock";
const RUNTIME_GATEWAY_REDIS_LEDGER_KEY: &str = "prodex:gateway:billing_ledger";
const RUNTIME_GATEWAY_REDIS_LEDGER_LOCK: &str = "prodex:gateway:billing_ledger:lock";
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
    pub(super) deepseek_conversations: RuntimeDeepSeekConversationStore,
    pub(super) deepseek_pending_messages: RuntimeDeepSeekPendingMessages,
    pub(super) gemini_conversations: RuntimeDeepSeekConversationStore,
    pub(super) gemini_oauth_pool: Option<RuntimeGeminiOAuthPool>,
    pub(super) copilot_oauth_pool: Option<RuntimeCopilotOAuthPool>,
    pub(super) model_memory: RuntimeLocalRewriteModelMemory,
    pub(super) api_key_cursor: Arc<AtomicUsize>,
    pub(super) client: reqwest::blocking::Client,
    pub(super) gateway_auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(super) gateway_admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(super) gateway_sso: RuntimeGatewaySsoConfig,
    pub(super) gateway_state_store: RuntimeGatewayStateStore,
    pub(super) gateway_virtual_keys: Arc<Mutex<Vec<RuntimeGatewayVirtualKeyEntry>>>,
    pub(super) gateway_virtual_key_store_path: PathBuf,
    pub(super) gateway_virtual_key_usage:
        Arc<Mutex<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>>>,
    pub(super) gateway_virtual_key_usage_path: Option<PathBuf>,
    gateway_virtual_key_usage_save_in_flight: Arc<AtomicBool>,
    gateway_virtual_key_usage_save_dirty: Arc<AtomicBool>,
    gateway_virtual_key_usage_pending_deltas: Arc<Mutex<Vec<RuntimeGatewayVirtualKeyUsageDelta>>>,
    gateway_virtual_key_request_ids: Arc<Mutex<BTreeSet<u64>>>,
    pub(super) gateway_route_aliases: Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>,
    pub(super) gateway_route_load: RuntimeGatewayRouteLoadState,
    pub(super) gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(super) gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(super) gateway_call_id_header: Option<String>,
    pub(super) gateway_observability: RuntimeGatewayObservabilityConfig,
}

pub(super) type RuntimeLocalRewriteModelMemory = Arc<Mutex<RuntimeLocalRewriteModelMemoryState>>;
pub(super) type RuntimeGatewayRouteLoadState =
    Arc<Mutex<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelState>>>;

pub(crate) fn start_runtime_local_rewrite_proxy(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
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
    let log_path = initialize_runtime_proxy_log_path();
    let bind_addr = preferred_listen_addr.unwrap_or("127.0.0.1:0");
    let server = Arc::new(TinyServer::http(bind_addr).map_err(|err| {
        anyhow::anyhow!("failed to bind runtime local rewrite proxy on {bind_addr}: {err}")
    })?);
    let listen_addr = server
        .server_addr()
        .to_ip()
        .context("runtime local rewrite proxy did not expose a TCP listen address")?;
    let worker_count = runtime_proxy_worker_count();
    let long_lived_worker_count = runtime_proxy_long_lived_worker_count();
    let active_request_limit =
        runtime_proxy_active_request_limit(worker_count, long_lived_worker_count);
    let lane_admission = RuntimeProxyLaneAdmission::new(runtime_proxy_lane_limits(
        active_request_limit,
        worker_count,
        long_lived_worker_count,
    ));
    let async_runtime = Arc::new(
        TokioRuntimeBuilder::new_multi_thread()
            .worker_threads(runtime_proxy_async_worker_count())
            .enable_all()
            .build()
            .context("failed to build runtime local rewrite async runtime")?,
    );
    let runtime_shared = RuntimeRotationProxyShared {
        upstream_no_proxy,
        async_client: build_runtime_upstream_async_http_client(true)?,
        async_runtime,
        log_path: log_path.clone(),
        request_sequence: Arc::new(AtomicU64::new(1)),
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
    let gateway_virtual_key_entries = runtime_gateway_virtual_key_entries_from_sources(
        gateway_virtual_keys,
        &gateway_state_store,
        &log_path,
    );
    let gateway_virtual_key_usage_path = Some(gateway_state_store.usage_path().to_path_buf());
    let gateway_virtual_key_usage =
        runtime_gateway_virtual_key_usage_load(&gateway_state_store, &log_path);
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
    let shared = RuntimeLocalRewriteProxyShared {
        runtime_shared: runtime_shared.clone(),
        upstream_base_url,
        mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        provider,
        deepseek_conversations: Arc::new(Mutex::new(BTreeMap::new())),
        deepseek_pending_messages: Arc::new(Mutex::new(BTreeMap::new())),
        gemini_conversations: Arc::new(Mutex::new(BTreeMap::new())),
        gemini_oauth_pool,
        copilot_oauth_pool,
        model_memory: Arc::new(Mutex::new(RuntimeLocalRewriteModelMemoryState::default())),
        api_key_cursor: Arc::new(AtomicUsize::new(0)),
        client: build_runtime_local_rewrite_http_client()?,
        gateway_auth_token_hash,
        gateway_admin_tokens,
        gateway_sso,
        gateway_state_store,
        gateway_virtual_keys: Arc::new(Mutex::new(gateway_virtual_key_entries)),
        gateway_virtual_key_store_path,
        gateway_virtual_key_usage: Arc::new(Mutex::new(gateway_virtual_key_usage)),
        gateway_virtual_key_usage_path,
        gateway_virtual_key_usage_save_in_flight: Arc::new(AtomicBool::new(false)),
        gateway_virtual_key_usage_save_dirty: Arc::new(AtomicBool::new(false)),
        gateway_virtual_key_usage_pending_deltas: Arc::new(Mutex::new(Vec::new())),
        gateway_virtual_key_request_ids: Arc::new(Mutex::new(BTreeSet::new())),
        gateway_route_aliases,
        gateway_route_load: Arc::new(Mutex::new(BTreeMap::new())),
        gateway_guardrails,
        gateway_guardrail_webhook,
        gateway_call_id_header,
        gateway_observability,
    };
    if let Some(pool) = shared.gemini_oauth_pool.as_ref() {
        pool.spawn_quota_refresh(shared.runtime_shared.log_path.clone());
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    let mut worker_threads = Vec::new();
    let gemini_live_sidecar_addr = if matches!(
        &shared.provider,
        RuntimeLocalRewriteProviderOptions::Gemini { .. }
    ) {
        Some(spawn_runtime_gemini_live_sidecar(
            shared.clone(),
            Arc::clone(&shutdown),
            &mut worker_threads,
        )?)
    } else {
        None
    };
    for _ in 0..worker_count {
        let server: Arc<TinyServer> = Arc::clone(&server);
        let shutdown = Arc::clone(&shutdown);
        let shared = shared.clone();
        worker_threads.push(thread::spawn(move || {
            loop {
                match server.recv() {
                    Ok(request) => {
                        let result = crate::runtime_panic::catch_runtime_unwind_silently(|| {
                            handle_runtime_local_rewrite_proxy_request(request, &shared);
                        });
                        if let Err(panic) = result {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                format!(
                                    "runtime_proxy_worker_panic lane=local_rewrite panic={}",
                                    crate::runtime_panic::runtime_panic_payload_label(
                                        panic.as_ref()
                                    )
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
        }));
    }

    Ok(RuntimeRotationProxy {
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
        owner_lock: None,
    })
}

fn build_runtime_local_rewrite_http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_millis(
            runtime_proxy_http_connect_timeout_ms(),
        ))
        .no_proxy()
        .build()
        .context("failed to build runtime local rewrite HTTP client")
}

fn handle_runtime_local_rewrite_proxy_request(
    request: tiny_http::Request,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let request_path = request.url().to_string();
    let runtime_shared = &shared.runtime_shared;
    let request_id = runtime_proxy_next_request_id(runtime_shared);
    if runtime_gateway_virtual_key_entries_is_empty(shared)
        && let Some(auth_token_hash) = shared.gateway_auth_token_hash.as_ref()
        && !runtime_gateway_request_path_is_admin(&request_path, shared)
        && !runtime_local_rewrite_request_is_authorized(&request, auth_token_hash)
    {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_auth_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("path", request_path.as_str()),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            401,
            "missing or invalid gateway bearer token",
        ));
        return;
    }
    let websocket = is_tiny_http_websocket_upgrade(&request);
    let request_transport = if websocket { "websocket" } else { "http" };
    let _active_request_guard = match acquire_runtime_proxy_active_request_slot_with_wait(
        runtime_shared,
        request_transport,
        &request_path,
    ) {
        Ok(guard) => guard,
        Err(RuntimeProxyAdmissionRejection::GlobalLimit) => {
            mark_runtime_proxy_local_overload(runtime_shared, "active_request_limit");
            reject_runtime_proxy_overloaded_request(
                request,
                runtime_shared,
                "active_request_limit",
            );
            return;
        }
        Err(RuntimeProxyAdmissionRejection::LaneLimit(lane)) => {
            let reason = format!("lane_limit:{}", runtime_route_kind_label(lane));
            reject_runtime_proxy_overloaded_request(request, runtime_shared, &reason);
            return;
        }
    };

    if websocket {
        if matches!(
            &shared.provider,
            RuntimeLocalRewriteProviderOptions::Gemini { .. }
        ) && is_runtime_realtime_websocket_path(&request_path)
        {
            handle_runtime_gemini_live_websocket_request(request_id, request, shared);
            return;
        }
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_websocket_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("path", request_path),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            501,
            "runtime local rewrite proxy does not support websocket upstreams",
        ));
        return;
    }

    let mut request = request;
    let mut captured = match capture_runtime_proxy_request(&mut request) {
        Ok(captured) => captured,
        Err(err) => {
            runtime_proxy_log(
                runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_capture_error",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            );
            let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
            return;
        }
    };
    if let Err(err) =
        apply_runtime_presidio_redaction_to_request(request_id, &mut captured, runtime_shared)
    {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_presidio_redaction_failed",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("error", format!("{err:#}")),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
        return;
    }
    if let Some(response) = runtime_gateway_admin_response(request_id, &captured, shared) {
        let _ = request.respond(response);
        return;
    }
    if let Some(redacted_body) = runtime_proxy_crate::runtime_gateway_redact_request_body(
        &captured.body,
        &shared.gateway_guardrails,
    ) {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_pii_redacted",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        captured.body = redacted_body;
    }
    if let Some(rejection) = runtime_gateway_virtual_key_rejection(request_id, &captured, shared) {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_virtual_key_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("reason", rejection.code()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_json_error_response(
            rejection.status(),
            rejection.code(),
            "gateway virtual key policy rejected this request",
        ));
        return;
    }
    if let Some(block) =
        runtime_gateway_guardrail_webhook_block("pre", request_id, &captured.body, shared)
    {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_webhook_blocked",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("phase", "pre"),
                    runtime_proxy_log_field("reason", block.reason.as_str()),
                    runtime_proxy_log_field("value", block.value.as_str()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_json_error_response(
            403,
            "policy_violation",
            "gateway guardrail webhook blocked this request",
        ));
        return;
    }
    if let Some(block) = runtime_proxy_crate::runtime_gateway_guardrail_block(
        &captured.body,
        &shared.gateway_guardrails,
    ) {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_blocked",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("reason", block.kind.as_str()),
                    runtime_proxy_log_field("value", block.value.as_str()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_json_error_response(
            403,
            "policy_violation",
            "gateway guardrail blocked this request",
        ));
        return;
    }
    let route_load = shared
        .gateway_route_load
        .lock()
        .map(|load| load.clone())
        .unwrap_or_default();
    let mut route_load_guard = None;
    if let Some(rewrite) = runtime_proxy_crate::runtime_gateway_rewrite_route_alias_with_state(
        &captured.body,
        &shared.gateway_route_aliases,
        request_id,
        &route_load,
    ) {
        route_load_guard = Some(RuntimeGatewayRouteLoadGuard::enter(
            Arc::clone(&shared.gateway_route_load),
            rewrite.model.as_str(),
            &captured.body,
        ));
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_route_alias_rewrite",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("alias", rewrite.alias.as_str()),
                    runtime_proxy_log_field("strategy", rewrite.strategy.as_str()),
                    runtime_proxy_log_field("model", rewrite.model.as_str()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        captured.body = rewrite.body;
    }
    if path_without_query(&captured.path_and_query).ends_with("/responses/compact") {
        if let RuntimeLocalRewriteProviderOptions::Gemini { auth, .. } = &shared.provider {
            respond_runtime_gemini_compact_request(request_id, request, &captured, shared, auth);
            return;
        }
        let provider_name = match &shared.provider {
            RuntimeLocalRewriteProviderOptions::Anthropic { .. } => "Anthropic",
            RuntimeLocalRewriteProviderOptions::Copilot { .. } => "GitHub Copilot",
            RuntimeLocalRewriteProviderOptions::Gemini { .. } => "Gemini",
            _ => "DeepSeek",
        };
        let _ = request.respond(build_runtime_proxy_text_response(
            501,
            &format!("{provider_name} provider does not support Codex remote compact yet"),
        ));
        return;
    }
    if let Some(parts) = runtime_provider_models_buffered_response(
        shared.provider.bridge_kind(),
        &captured.method,
        &captured.path_and_query,
    ) {
        runtime_proxy_log(
            runtime_shared,
            runtime_provider_request_ledger_message(
                request_id,
                shared.provider.bridge_kind(),
                &captured.path_and_query,
                None,
                parts.status,
                0,
                captured.body.len(),
            ),
        );
        let _ = request.respond(runtime_local_rewrite_response_with_call_id(
            parts, request_id, shared,
        ));
        return;
    }
    let response = match send_runtime_local_rewrite_upstream_request(request_id, &captured, shared)
    {
        Ok(response) => response,
        Err(err) => {
            runtime_proxy_log(
                runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_upstream_error",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            );
            let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
            return;
        }
    };
    respond_runtime_local_rewrite_proxy_request(request_id, request, response, &captured, shared);
    drop(route_load_guard);
}

fn runtime_gateway_virtual_key_entries_is_empty(shared: &RuntimeLocalRewriteProxyShared) -> bool {
    shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.iter().all(|entry| entry.disabled))
        .unwrap_or(true)
}

fn runtime_gateway_active_virtual_keys(
    shared: &RuntimeLocalRewriteProxyShared,
) -> Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey> {
    shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| !entry.disabled)
                .map(|entry| entry.key.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn runtime_gateway_virtual_key_entries_from_sources(
    policy_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> Vec<RuntimeGatewayVirtualKeyEntry> {
    let mut entries = policy_keys
        .into_iter()
        .map(|key| RuntimeGatewayVirtualKeyEntry {
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
    for record in runtime_gateway_virtual_key_store_load(state_store, log_path).keys {
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
                    [
                        runtime_proxy_log_field(
                            "path",
                            state_store.key_store_path().display().to_string(),
                        ),
                        runtime_proxy_log_field("key", key_name),
                    ],
                ),
            );
            continue;
        }
        let Some(entry) = runtime_gateway_virtual_key_entry_from_stored(&record) else {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_store_invalid_hash",
                    [
                        runtime_proxy_log_field(
                            "path",
                            state_store.key_store_path().display().to_string(),
                        ),
                        runtime_proxy_log_field("key", key_name),
                    ],
                ),
            );
            continue;
        };
        seen.push(normalized);
        entries.push(entry);
    }
    entries
}

pub(super) fn runtime_gateway_virtual_key_store_load(
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> RuntimeGatewayVirtualKeyStoreFile {
    let path = state_store.key_store_path();
    match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => {
            return match runtime_gateway_sqlite_load_key_store(path) {
                Ok(mut store) => {
                    store.sort_for_rendering();
                    store
                }
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_store_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    RuntimeGatewayVirtualKeyStoreFile::default()
                }
            };
        }
        RuntimeGatewayStateStore::Postgres {
            url, state_path, ..
        } => {
            return match runtime_gateway_postgres_load_key_store(url) {
                Ok(mut store) => {
                    store.sort_for_rendering();
                    store
                }
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_store_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", state_path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    RuntimeGatewayVirtualKeyStoreFile::default()
                }
            };
        }
        RuntimeGatewayStateStore::Redis { url, state_path } => {
            return match runtime_gateway_redis_load_key_store(
                url,
                RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY,
            ) {
                Ok(mut store) => {
                    store.sort_for_rendering();
                    store
                }
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_store_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", state_path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    RuntimeGatewayVirtualKeyStoreFile::default()
                }
            };
        }
        RuntimeGatewayStateStore::File { .. } => {}
    }
    match runtime_gateway_virtual_key_store_file_load(path) {
        Ok(mut store) => {
            store.sort_for_rendering();
            store
        }
        Err(err) => {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_store_load_failed",
                    [
                        runtime_proxy_log_field("path", path.display().to_string()),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
            RuntimeGatewayVirtualKeyStoreFile::default()
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
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            runtime_gateway_postgres_ledger_load(url, limit).map_err(std::io::Error::other)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_redis_ledger_load(url, RUNTIME_GATEWAY_REDIS_LEDGER_KEY, limit)
                .map_err(std::io::Error::other)
        }
    }
}

pub(super) fn schedule_runtime_gateway_billing_ledger_reconcile(
    shared: &RuntimeLocalRewriteProxyShared,
    event: RuntimeProviderGatewaySpendEvent,
) {
    if event.phase != "response" {
        return;
    }
    let should_reconcile = shared
        .gateway_virtual_key_request_ids
        .lock()
        .map(|request_ids| request_ids.contains(&event.request))
        .unwrap_or(false);
    if !should_reconcile {
        return;
    }
    let state_store = shared.gateway_state_store.clone();
    let runtime_shared = shared.runtime_shared.clone();
    let request_ids = Arc::clone(&shared.gateway_virtual_key_request_ids);
    shared.runtime_shared.async_runtime.spawn_blocking(move || {
        let mut last_error = None;
        for attempt in 0..25 {
            match runtime_gateway_billing_ledger_reconcile_response(&state_store, &event) {
                Ok(true) => {
                    if let Ok(mut request_ids) = request_ids.lock() {
                        request_ids.remove(&event.request);
                    }
                    return;
                }
                Ok(false) => {
                    if attempt < 24 {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                }
                Err(err) => {
                    last_error = Some(err);
                    break;
                }
            }
        }
        if let Ok(mut request_ids) = request_ids.lock() {
            request_ids.remove(&event.request);
        }
        if let Some(err) = last_error {
            crate::runtime_proxy_log(
                &runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_billing_ledger_reconcile_failed",
                    [
                        runtime_proxy_log_field("request", event.request.to_string()),
                        runtime_proxy_log_field("backend", state_store.label()),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
        }
    });
}

fn runtime_gateway_billing_ledger_reconcile_response(
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
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            runtime_gateway_postgres_ledger_reconcile_response(
                url,
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

fn runtime_gateway_virtual_key_rejection(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection> {
    if path_without_query(&captured.path_and_query) == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH
    {
        return None;
    }
    if let Some(auth_token_hash) = shared.gateway_auth_token_hash.as_ref()
        && captured.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("authorization")
                && auth_token_hash.verify_authorization_header(value)
        })
    {
        return None;
    }
    let active_keys = runtime_gateway_active_virtual_keys(shared);
    let key = match runtime_proxy_crate::runtime_gateway_virtual_key_from_headers(
        &captured.headers,
        &active_keys,
    ) {
        Ok(Some(key)) => key,
        Ok(None) => return None,
        Err(rejection) => return Some(rejection),
    };
    let model = runtime_proxy_crate::runtime_gateway_request_model(&captured.body)
        .unwrap_or_else(|| "unknown".to_string());
    let input_tokens = estimate_request_input_tokens(&captured.body);
    let route_load = shared
        .gateway_route_load
        .lock()
        .map(|load| load.clone())
        .unwrap_or_default();
    let cost = runtime_provider_gateway_cost_for_request(
        shared.provider.bridge_kind(),
        &shared.gateway_route_aliases,
        &route_load,
        request_id,
        &captured.body,
        &model,
    );
    let estimated_cost_microusd = calculate_cost_microusd(Some(input_tokens), None, cost);
    let minute_epoch = runtime_proxy_crate::runtime_gateway_minute_epoch();
    let usage_map = shared
        .gateway_virtual_key_usage
        .lock()
        .map(|usage| usage.clone())
        .unwrap_or_default();
    if let Some(rejection) = runtime_gateway_budget_group_rejection(
        key,
        &active_keys,
        &usage_map,
        estimated_cost_microusd,
    ) {
        return Some(rejection);
    }
    let usage = usage_map.get(&key.name).cloned();
    let admission = match runtime_proxy_crate::runtime_gateway_virtual_key_admission(
        key,
        usage.as_ref(),
        &captured.body,
        estimated_cost_microusd,
        minute_epoch,
    ) {
        Ok(admission) => admission,
        Err(rejection) => return Some(rejection),
    };
    if let Ok(mut usage) = shared.gateway_virtual_key_usage.lock() {
        let entry = usage.entry(admission.key_name.clone()).or_default();
        runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
            entry,
            &admission,
            minute_epoch,
        );
    }
    if let Ok(mut request_ids) = shared.gateway_virtual_key_request_ids.lock() {
        request_ids.insert(request_id);
    }
    schedule_runtime_gateway_virtual_key_usage_save(
        shared,
        RuntimeGatewayVirtualKeyUsageDelta {
            request_id,
            key_name: admission.key_name.clone(),
            model: model.clone(),
            minute_epoch,
            input_tokens: admission.input_tokens,
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
    None
}

fn runtime_gateway_virtual_key_usage_load(
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage> {
    let path = state_store.usage_path();
    match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => {
            return match runtime_gateway_sqlite_usage_load(path) {
                Ok(usage) => usage,
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_usage_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    BTreeMap::new()
                }
            };
        }
        RuntimeGatewayStateStore::Postgres {
            url, state_path, ..
        } => {
            return match runtime_gateway_postgres_usage_load(url) {
                Ok(usage) => usage,
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_usage_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", state_path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    BTreeMap::new()
                }
            };
        }
        RuntimeGatewayStateStore::Redis { url, state_path } => {
            return match runtime_gateway_redis_usage_load(url, RUNTIME_GATEWAY_REDIS_USAGE_KEY) {
                Ok(usage) => usage,
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_usage_load_failed",
                            [
                                runtime_proxy_log_field("backend", state_store.label()),
                                runtime_proxy_log_field("path", state_path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    BTreeMap::new()
                }
            };
        }
        RuntimeGatewayStateStore::File { .. } => {}
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            match serde_json::from_slice::<
                BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
            >(&bytes)
            {
                Ok(usage) => usage,
                Err(err) => {
                    runtime_proxy_log_to_path(
                        log_path,
                        &runtime_proxy_structured_log_message(
                            "gateway_virtual_key_usage_load_failed",
                            [
                                runtime_proxy_log_field("path", path.display().to_string()),
                                runtime_proxy_log_field("error", err.to_string()),
                            ],
                        ),
                    );
                    BTreeMap::new()
                }
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(err) => {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_usage_load_failed",
                    [
                        runtime_proxy_log_field("path", path.display().to_string()),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
            BTreeMap::new()
        }
    }
}

fn schedule_runtime_gateway_virtual_key_usage_save(
    shared: &RuntimeLocalRewriteProxyShared,
    delta: RuntimeGatewayVirtualKeyUsageDelta,
) {
    let state_store = shared.gateway_state_store.clone();
    if let Ok(mut pending) = shared.gateway_virtual_key_usage_pending_deltas.lock() {
        pending.push(delta);
    } else {
        return;
    }
    shared
        .gateway_virtual_key_usage_save_dirty
        .store(true, Ordering::Release);
    if shared
        .gateway_virtual_key_usage_save_in_flight
        .swap(true, Ordering::AcqRel)
    {
        return;
    }

    let pending_deltas = Arc::clone(&shared.gateway_virtual_key_usage_pending_deltas);
    let dirty = Arc::clone(&shared.gateway_virtual_key_usage_save_dirty);
    let in_flight = Arc::clone(&shared.gateway_virtual_key_usage_save_in_flight);
    let log_path = shared.runtime_shared.log_path.clone();
    drop(shared.runtime_shared.async_runtime.spawn_blocking(move || {
        loop {
            dirty.store(false, Ordering::Release);
            let deltas = pending_deltas
                .lock()
                .map(|mut pending| pending.drain(..).collect::<Vec<_>>())
                .unwrap_or_default();
            if !deltas.is_empty()
                && let Err(err) =
                    runtime_gateway_virtual_key_usage_apply_deltas(&state_store, &deltas)
            {
                runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "gateway_virtual_key_usage_save_failed",
                        [
                            runtime_proxy_log_field("backend", state_store.label()),
                            runtime_proxy_log_field(
                                "path",
                                state_store.usage_path().display().to_string(),
                            ),
                            runtime_proxy_log_field("error", err.to_string()),
                        ],
                    ),
                );
            }
            if !dirty.load(Ordering::Acquire) {
                in_flight.store(false, Ordering::Release);
                if dirty.load(Ordering::Acquire) && !in_flight.swap(true, Ordering::AcqRel) {
                    continue;
                }
                break;
            }
        }
    }));
}

pub(super) fn runtime_gateway_virtual_key_usage_apply_deltas(
    state_store: &RuntimeGatewayStateStore,
    deltas: &[RuntimeGatewayVirtualKeyUsageDelta],
) -> std::io::Result<()> {
    let path = state_store.usage_path();
    match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => {
            return runtime_gateway_sqlite_usage_apply_deltas(path, deltas)
                .map_err(std::io::Error::other);
        }
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            return runtime_gateway_postgres_usage_apply_deltas(url, deltas)
                .map_err(std::io::Error::other);
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            return runtime_gateway_redis_usage_apply_deltas(
                url,
                RUNTIME_GATEWAY_REDIS_USAGE_KEY,
                RUNTIME_GATEWAY_REDIS_USAGE_LOCK,
                RUNTIME_GATEWAY_REDIS_LEDGER_KEY,
                RUNTIME_GATEWAY_REDIS_LEDGER_LOCK,
                runtime_gateway_generate_virtual_key_token,
                deltas,
            )
            .map_err(std::io::Error::other);
        }
        RuntimeGatewayStateStore::File { .. } => {}
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_path = path.with_extension("json.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    lock_file.lock_exclusive()?;
    let mut usage = runtime_gateway_virtual_key_usage_load_strict(path)?;
    for delta in deltas {
        let entry = usage.entry(delta.key_name.clone()).or_default();
        let admission = runtime_proxy_crate::RuntimeGatewayVirtualKeyAdmission {
            key_name: delta.key_name.clone(),
            model: None,
            input_tokens: delta.input_tokens,
            estimated_cost_microusd: delta.estimated_cost_microusd,
        };
        runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
            entry,
            &admission,
            delta.minute_epoch,
        );
    }
    let payload = serde_json::to_vec_pretty(&usage).map_err(std::io::Error::other)?;
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, payload)?;
    std::fs::rename(tmp_path, path)?;
    runtime_gateway_file_ledger_append_deltas(state_store.ledger_path(), deltas)?;
    let _ = lock_file.unlock();
    Ok(())
}

fn runtime_gateway_virtual_key_usage_load_strict(
    path: &Path,
) -> std::io::Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<
            BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
        >(&bytes)
        .map_err(std::io::Error::other),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(err) => Err(err),
    }
}

fn runtime_local_rewrite_request_is_authorized(
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
