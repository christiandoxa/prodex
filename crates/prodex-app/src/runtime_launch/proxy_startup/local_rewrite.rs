use super::super::copilot_instructions::runtime_copilot_init_current_workspace_custom_instructions;
use super::deepseek_rewrite::*;
use super::local_rewrite_copilot::{
    RuntimeCopilotOAuthPool, runtime_copilot_model_catalog_from_provider,
    runtime_copilot_oauth_pool_from_provider,
};
use super::local_rewrite_gateway_admin_router::{
    runtime_gateway_admin_response, runtime_gateway_request_path_is_admin,
};
pub(crate) use super::local_rewrite_gateway_config::{
    RuntimeGatewayAdminRole, RuntimeGatewayAdminToken, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewayOidcConfig, RuntimeGatewaySsoConfig,
    RuntimeGatewayStateStore,
};
pub(super) use super::local_rewrite_gateway_guardrail_webhook::runtime_gateway_guardrail_webhook_block;
pub(super) use super::local_rewrite_gateway_keys::{
    runtime_gateway_virtual_key_entries_from_sources, runtime_gateway_virtual_key_entries_is_empty,
    runtime_gateway_virtual_key_rejection, runtime_gateway_virtual_key_store_load,
    runtime_local_rewrite_request_is_authorized,
};
pub(super) use super::local_rewrite_gateway_ledger::{
    runtime_gateway_billing_ledger_load, schedule_runtime_gateway_billing_ledger_reconcile,
};
use super::local_rewrite_gateway_route_load::RuntimeGatewayRouteLoadGuard;
use super::local_rewrite_gateway_store_types::RuntimeGatewayVirtualKeyEntry;
#[cfg(test)]
pub(super) use super::local_rewrite_gateway_usage::runtime_gateway_virtual_key_usage_apply_deltas;
pub(super) use super::local_rewrite_gateway_usage::{
    runtime_gateway_virtual_key_usage_load, schedule_runtime_gateway_virtual_key_usage_save,
};
pub(super) use super::local_rewrite_gateway_usage_backend::RuntimeGatewayVirtualKeyUsageDelta;
pub(super) use super::local_rewrite_gateway_util::runtime_gateway_generate_virtual_key_token;
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
    runtime_provider_models_buffered_response, runtime_provider_openai_contract,
    runtime_provider_request_ledger_message,
};
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH: &str = "/v1";
pub(super) const RUNTIME_LOCAL_REWRITE_PROFILE: &str = "local";
pub(super) const RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY: &str = "prodex:gateway:virtual_keys";
pub(super) const RUNTIME_GATEWAY_REDIS_KEY_STORE_LOCK: &str = "prodex:gateway:virtual_keys:lock";
pub(super) const RUNTIME_GATEWAY_REDIS_LEDGER_KEY: &str = "prodex:gateway:billing_ledger";
pub(super) const RUNTIME_GATEWAY_REDIS_LEDGER_LOCK: &str = "prodex:gateway:billing_ledger:lock";
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
    pub(super) gateway_usage: RuntimeGatewayVirtualKeyUsageState,
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

#[derive(Clone)]
pub(super) struct RuntimeGatewayVirtualKeyUsageState {
    pub(super) usage:
        Arc<Mutex<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>>>,
    pub(super) path: Option<PathBuf>,
    pub(super) save_in_flight: Arc<AtomicBool>,
    pub(super) save_dirty: Arc<AtomicBool>,
    pub(super) pending_deltas: Arc<Mutex<Vec<RuntimeGatewayVirtualKeyUsageDelta>>>,
    pub(super) request_ids: Arc<Mutex<BTreeSet<u64>>>,
}

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
        auto_redeem_enabled: false,
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
    let gateway_virtual_key_usage_path = gateway_state_store.usage_path().to_path_buf();
    let gateway_virtual_key_entries = runtime_gateway_virtual_key_entries_from_sources(
        gateway_virtual_keys,
        &gateway_state_store,
        &log_path,
    );
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
        gateway_usage: RuntimeGatewayVirtualKeyUsageState {
            usage: Arc::new(Mutex::new(gateway_virtual_key_usage)),
            path: Some(gateway_virtual_key_usage_path),
            save_in_flight: Arc::new(AtomicBool::new(false)),
            save_dirty: Arc::new(AtomicBool::new(false)),
            pending_deltas: Arc::new(Mutex::new(Vec::new())),
            request_ids: Arc::new(Mutex::new(BTreeSet::new())),
        },
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
            RuntimeLocalRewriteProviderOptions::OpenAiResponses { .. } => "OpenAI",
            RuntimeLocalRewriteProviderOptions::LocalEmbeddingsOnly { .. } => {
                "Prodex local embeddings"
            }
            RuntimeLocalRewriteProviderOptions::Gemini { .. } => "Gemini",
            RuntimeLocalRewriteProviderOptions::DeepSeek { .. } => "DeepSeek",
        };
        let _ = request.respond(build_runtime_proxy_text_response(
            501,
            &format!("{provider_name} provider does not support Codex remote compact yet"),
        ));
        return;
    }
    let dynamic_model_catalog = runtime_copilot_model_catalog_from_provider(&shared.provider);
    if let Some(parts) = runtime_provider_models_buffered_response(
        shared.provider.bridge_kind(),
        (!dynamic_model_catalog.is_empty()).then_some(dynamic_model_catalog.as_slice()),
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
