use super::super::copilot_instructions::runtime_copilot_init_current_workspace_custom_instructions;
use super::deepseek_rewrite::*;
use super::gemini_rewrite::*;
use super::local_rewrite_copilot::{
    RuntimeCopilotOAuthPool, RuntimeCopilotRequestContext,
    runtime_copilot_oauth_pool_from_provider, send_runtime_copilot_upstream_request,
};
use super::local_rewrite_deepseek::send_runtime_deepseek_upstream_request;
use super::local_rewrite_gemini::{
    RuntimeGeminiOAuthPool, RuntimeGeminiRequestContext, runtime_gemini_oauth_pool_from_provider,
    send_runtime_gemini_upstream_request,
};
use super::local_rewrite_gemini_compact::{
    runtime_gemini_local_compact_response_parts, runtime_gemini_semantic_compact_request_body,
    runtime_gemini_semantic_compact_response_parts,
};
use super::local_rewrite_gemini_live::{
    handle_runtime_gemini_live_websocket_request, spawn_runtime_gemini_live_sidecar,
};
use super::local_rewrite_response::{
    respond_runtime_local_rewrite_proxy_request,
    runtime_local_rewrite_buffered_response_from_response,
    runtime_local_rewrite_response_with_call_id,
};
use super::local_rewrite_search_fallback::{
    RuntimeLocalRewritePreparedSendResult, RuntimeLocalRewriteSearchFallbackRequest,
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback,
};
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_local_rewrite_anthropic_auth_attempts,
    runtime_local_rewrite_api_key_attempts, runtime_local_rewrite_upstream_url,
    runtime_openai_standard_provider_upstream_url, send_runtime_local_rewrite_prepared_request,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_error_class, runtime_provider_model_fallback_chain,
    runtime_provider_model_from_body, runtime_provider_models_buffered_response,
    runtime_provider_openai_contract, runtime_provider_request_body_with_model,
    runtime_provider_request_ledger_message, runtime_provider_should_retry_with_next_model,
    runtime_provider_should_rotate_auth_after_response,
};
use super::*;
use base64::Engine;

pub(crate) const RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH: &str = "/v1";
pub(super) const RUNTIME_LOCAL_REWRITE_PROFILE: &str = "local";

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

#[derive(Debug, Default)]
pub(super) struct RuntimeLocalRewriteModelMemoryState {
    selected_models: BTreeMap<String, String>,
}

pub(super) struct RuntimeLocalRewriteModelSelection {
    pub(super) model: String,
    pub(super) body: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeGatewayObservabilityConfig {
    pub(crate) sinks: Vec<String>,
    pub(crate) jsonl_path: Option<PathBuf>,
    pub(crate) http_endpoint: Option<String>,
    pub(crate) http_schema: String,
    pub(crate) http_bearer_token: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeGatewayGuardrailWebhookConfig {
    pub(crate) url: Option<String>,
    pub(crate) phases: Vec<String>,
    pub(crate) bearer_token: Option<String>,
    pub(crate) fail_closed: bool,
}

impl RuntimeGatewayGuardrailWebhookConfig {
    pub(super) fn enabled_for(&self, phase: &str) -> bool {
        self.url.is_some()
            && (self.phases.is_empty()
                || self
                    .phases
                    .iter()
                    .any(|configured| configured.eq_ignore_ascii_case(phase)))
    }
}

impl RuntimeGatewayObservabilityConfig {
    pub(super) fn sink_enabled(&self, sink: &str) -> bool {
        self.sinks
            .iter()
            .any(|configured| configured.eq_ignore_ascii_case(sink))
    }
}

#[derive(Clone)]
pub(crate) enum RuntimeLocalRewriteProviderOptions {
    Anthropic {
        auth: RuntimeAnthropicProviderAuth,
    },
    Copilot {
        auth: RuntimeCopilotProviderAuth,
    },
    OpenAiResponses {
        api_keys: Vec<String>,
    },
    DeepSeek {
        api_keys: Vec<String>,
    },
    Gemini {
        auth: RuntimeGeminiProviderAuth,
        thinking_budget_tokens: Option<u64>,
        model_resolution: crate::RuntimeGeminiModelResolution,
    },
}

impl RuntimeLocalRewriteProviderOptions {
    pub(super) fn bridge_kind(&self) -> RuntimeProviderBridgeKind {
        match self {
            RuntimeLocalRewriteProviderOptions::Anthropic { .. } => {
                RuntimeProviderBridgeKind::Anthropic
            }
            RuntimeLocalRewriteProviderOptions::Copilot { .. } => {
                RuntimeProviderBridgeKind::Copilot
            }
            RuntimeLocalRewriteProviderOptions::OpenAiResponses { .. } => {
                RuntimeProviderBridgeKind::OpenAiResponses
            }
            RuntimeLocalRewriteProviderOptions::DeepSeek { .. } => {
                RuntimeProviderBridgeKind::DeepSeek
            }
            RuntimeLocalRewriteProviderOptions::Gemini { .. } => RuntimeProviderBridgeKind::Gemini,
        }
    }
}

pub(crate) struct RuntimeLocalRewriteProxyStartOptions<'a> {
    pub(crate) paths: &'a AppPaths,
    pub(crate) state: &'a AppState,
    pub(crate) upstream_base_url: String,
    pub(crate) provider: RuntimeLocalRewriteProviderOptions,
    pub(crate) upstream_no_proxy: bool,
    pub(crate) smart_context_enabled: bool,
    pub(crate) presidio_redaction_enabled: bool,
    pub(crate) model_context_window_tokens: Option<u64>,
    pub(crate) preferred_listen_addr: Option<&'a str>,
    pub(crate) gateway_auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(crate) gateway_route_aliases: Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>,
    pub(crate) gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(crate) gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(crate) gateway_call_id_header: Option<String>,
    pub(crate) gateway_observability: RuntimeGatewayObservabilityConfig,
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
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime local rewrite proxy started listen_addr={listen_addr} smart_context_enabled={smart_context_enabled} presidio_redaction_enabled={presidio_redaction_enabled} upstream_base_url={upstream_base_url} upstream_proxy_mode={} provider={} client_format={} upstream_format={} response_format={} endpoint={} auth_required={} route_aliases={} guardrail_blocked_keywords={} guardrail_blocked_output_keywords={} guardrail_allowed_models={} observability_sinks={}",
            runtime_upstream_proxy_mode_label(true),
            super::provider_bridge::runtime_provider_label(bridge_kind),
            openai_contract.client_request_format.label(),
            openai_contract.upstream_request_format.label(),
            openai_contract.response_format.label(),
            openai_contract.canonical_client_endpoint,
            gateway_auth_token_hash.is_some(),
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

pub(super) fn runtime_local_rewrite_model_selection(
    shared: &RuntimeLocalRewriteProxyShared,
    provider_kind: RuntimeProviderBridgeKind,
    request: &RuntimeProxyRequest,
    body: &[u8],
    default_model: &str,
) -> RuntimeLocalRewriteModelSelection {
    let body_model = runtime_provider_model_from_body(body);
    let original_model = body_model
        .clone()
        .unwrap_or_else(|| default_model.to_string());
    let scope = runtime_local_rewrite_model_scope(provider_kind, request, body);
    let remembered_model = scope
        .as_deref()
        .and_then(|scope| shared.model_memory.lock().ok()?.selected_model(scope));
    let model = remembered_model
        .filter(|_| runtime_local_rewrite_model_allows_session_memory(&original_model))
        .unwrap_or_else(|| original_model.clone());
    if let (Some(scope), Some(explicit_model)) = (scope.as_deref(), body_model.as_deref())
        && !runtime_local_rewrite_model_allows_session_memory(explicit_model)
        && let Ok(mut memory) = shared.model_memory.lock()
    {
        memory.remember_selected_model(scope, explicit_model);
    }
    let body = if model != original_model {
        runtime_provider_request_body_with_model(body, &model)
    } else {
        body.to_vec()
    };
    RuntimeLocalRewriteModelSelection { model, body }
}

pub(super) fn runtime_local_rewrite_model_allows_session_memory(model: &str) -> bool {
    matches!(
        model.trim().to_ascii_lowercase().as_str(),
        "" | "auto" | "default"
    )
}

pub(super) fn runtime_local_rewrite_model_scope(
    provider_kind: RuntimeProviderBridgeKind,
    request: &RuntimeProxyRequest,
    body: &[u8],
) -> Option<String> {
    runtime_proxy_crate::runtime_request_explicit_session_id(request)
        .map(runtime_proxy_crate::RuntimeExplicitSessionId::into_string)
        .or_else(|| runtime_proxy_crate::runtime_request_session_id_from_turn_metadata(request))
        .or_else(|| {
            serde_json::from_slice::<serde_json::Value>(body)
                .ok()
                .and_then(|value| {
                    runtime_proxy_crate::runtime_request_session_id_from_value(&value)
                })
        })
        .map(|session_id| {
            format!(
                "{}:session:{session_id}",
                super::provider_bridge::runtime_provider_label(provider_kind)
            )
        })
}

impl RuntimeLocalRewriteModelMemoryState {
    pub(super) fn remember_selected_model(&mut self, scope: &str, model: &str) {
        self.selected_models
            .insert(scope.to_string(), model.trim().to_string());
    }

    pub(super) fn selected_model(&self, scope: &str) -> Option<String> {
        self.selected_models.get(scope).cloned()
    }
}

fn handle_runtime_local_rewrite_proxy_request(
    request: tiny_http::Request,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let request_path = request.url().to_string();
    let runtime_shared = &shared.runtime_shared;
    let request_id = runtime_proxy_next_request_id(runtime_shared);
    if let Some(auth_token_hash) = shared.gateway_auth_token_hash.as_ref()
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
            captured.body.len(),
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

struct RuntimeGatewayRouteLoadGuard {
    load: RuntimeGatewayRouteLoadState,
    model: String,
    started_at: Instant,
}

impl RuntimeGatewayRouteLoadGuard {
    fn enter(load: RuntimeGatewayRouteLoadState, model: &str, body_bytes: usize) -> Self {
        let minute_epoch = runtime_gateway_route_minute_epoch();
        let estimated_tokens = (body_bytes as u64 / 4).saturating_add(1);
        if let Ok(mut load_map) = load.lock() {
            let entry = load_map.entry(model.to_string()).or_default();
            if entry.minute_epoch != minute_epoch {
                entry.minute_epoch = minute_epoch;
                entry.requests_this_minute = 0;
                entry.tokens_this_minute = 0;
            }
            entry.in_flight = entry.in_flight.saturating_add(1);
            entry.requests_this_minute = entry.requests_this_minute.saturating_add(1);
            entry.tokens_this_minute = entry.tokens_this_minute.saturating_add(estimated_tokens);
        }
        Self {
            load,
            model: model.to_string(),
            started_at: Instant::now(),
        }
    }
}

impl Drop for RuntimeGatewayRouteLoadGuard {
    fn drop(&mut self) {
        if let Ok(mut load_map) = self.load.lock()
            && let Some(state) = load_map.get_mut(&self.model)
        {
            state.in_flight = state.in_flight.saturating_sub(1);
            let elapsed_ms = self
                .started_at
                .elapsed()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64;
            state.latency_ms_ewma = Some(match state.latency_ms_ewma {
                Some(previous) => previous.saturating_mul(7).saturating_add(elapsed_ms) / 8,
                None => elapsed_ms,
            });
            if state.in_flight == 0
                && state.requests_this_minute == 0
                && state.tokens_this_minute == 0
            {
                load_map.remove(&self.model);
            }
        }
    }
}

fn runtime_gateway_route_minute_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 60)
        .unwrap_or_default()
}

fn runtime_local_rewrite_request_is_authorized(
    request: &tiny_http::Request,
    auth_token_hash: &runtime_proxy_crate::LocalBridgeBearerTokenHash,
) -> bool {
    if path_without_query(request.url()) == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH {
        return true;
    }
    request.headers().iter().any(|header| {
        header.field.equiv("Authorization")
            && auth_token_hash.verify_authorization_header(header.value.as_str())
    })
}

pub(super) struct RuntimeGatewayGuardrailWebhookBlock {
    pub(super) reason: String,
    pub(super) value: String,
}

pub(super) fn runtime_gateway_guardrail_webhook_block(
    phase: &str,
    request_id: u64,
    body: &[u8],
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayGuardrailWebhookBlock> {
    if !shared.gateway_guardrail_webhook.enabled_for(phase) {
        return None;
    }
    let url = shared.gateway_guardrail_webhook.url.as_deref()?;
    let payload = serde_json::json!({
        "phase": phase,
        "request_id": request_id,
        "call_id": format!("prodex-{request_id}"),
        "body_base64": base64::engine::general_purpose::STANDARD.encode(body),
    });
    let mut request = shared.client.post(url).json(&payload);
    if let Some(token) = shared.gateway_guardrail_webhook.bearer_token.as_deref() {
        request = request.bearer_auth(token);
    }
    let response = match request.send() {
        Ok(response) => response,
        Err(err) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_guardrail_webhook_failed",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("phase", phase),
                        runtime_proxy_log_field("url", url),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            );
            return shared.gateway_guardrail_webhook.fail_closed.then(|| {
                RuntimeGatewayGuardrailWebhookBlock {
                    reason: "webhook_error".to_string(),
                    value: err.to_string(),
                }
            });
        }
    };
    let status = response.status();
    let body = response.bytes().unwrap_or_default();
    if !status.is_success() {
        return shared.gateway_guardrail_webhook.fail_closed.then(|| {
            RuntimeGatewayGuardrailWebhookBlock {
                reason: "webhook_status".to_string(),
                value: status.as_u16().to_string(),
            }
        });
    }
    let value = serde_json::from_slice::<serde_json::Value>(&body).ok()?;
    let allow = value
        .get("allow")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    (!allow).then(|| RuntimeGatewayGuardrailWebhookBlock {
        reason: value
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("webhook_denied")
            .to_string(),
        value: value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("blocked")
            .to_string(),
    })
}

fn respond_runtime_gemini_compact_request(
    request_id: u64,
    request: tiny_http::Request,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    auth: &RuntimeGeminiProviderAuth,
) {
    let semantic = runtime_gemini_semantic_compact_request_body(&captured.body).and_then(|body| {
        let semantic_request = RuntimeProxyRequest {
            method: captured.method.clone(),
            path_and_query: format!("{}/responses", shared.mount_path.trim_end_matches('/')),
            headers: captured.headers.clone(),
            body: body.clone(),
        };
        let result = send_runtime_gemini_upstream_request(
            request_id,
            &semantic_request,
            shared,
            body,
            auth,
        )?;
        let RuntimeLocalRewriteUpstreamResult {
            response,
            gemini_context,
            ..
        } = result;
        let profile_name = gemini_context
            .as_ref()
            .map(|context| context.profile_name.as_str())
            .unwrap_or(RUNTIME_LOCAL_REWRITE_PROFILE);
        let parts = match response {
            RuntimeLocalRewriteUpstreamResponse::Live(live) if live.prefix.is_empty() => {
                let status = live.response.status().as_u16();
                runtime_gemini_semantic_compact_response_parts(
                    status,
                    live.response,
                    request_id,
                    &captured.body,
                )?
            }
            RuntimeLocalRewriteUpstreamResponse::Live(_) => {
                anyhow::bail!("Gemini semantic compact unexpectedly returned a stream prefix")
            }
            RuntimeLocalRewriteUpstreamResponse::Buffered(parts) => {
                anyhow::bail!(
                    "Gemini semantic compact returned buffered HTTP {}",
                    parts.status
                )
            }
        };
        Ok((parts, profile_name.to_string()))
    });

    let parts = match semantic {
        Ok((parts, profile_name)) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_gemini_compact_semantic",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("profile", profile_name),
                        runtime_proxy_log_field(
                            "path",
                            path_without_query(&captured.path_and_query),
                        ),
                        runtime_proxy_log_field("body_bytes", captured.body.len().to_string()),
                    ],
                ),
            );
            parts
        }
        Err(err) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_gemini_compact_fallback",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field(
                            "path",
                            path_without_query(&captured.path_and_query),
                        ),
                        runtime_proxy_log_field("body_bytes", captured.body.len().to_string()),
                        runtime_proxy_log_field("reason", format!("{err:#}")),
                    ],
                ),
            );
            runtime_gemini_local_compact_response_parts(&captured.body)
        }
    };
    let _ = request.respond(runtime_local_rewrite_response_with_call_id(
        parts, request_id, shared,
    ));
}

pub(super) struct RuntimeLocalRewriteUpstreamResult {
    pub(super) response: RuntimeLocalRewriteUpstreamResponse,
    pub(super) gemini_context: Option<RuntimeGeminiRequestContext>,
    pub(super) copilot_context: Option<RuntimeCopilotRequestContext>,
}

pub(super) enum RuntimeLocalRewriteUpstreamResponse {
    Live(RuntimeLocalRewriteLiveResponse),
    Buffered(RuntimeHeapTrimmedBufferedResponseParts),
}

pub(super) struct RuntimeLocalRewriteLiveResponse {
    pub(super) response: reqwest::blocking::Response,
    pub(super) prefix: Vec<u8>,
}

impl RuntimeLocalRewriteLiveResponse {
    pub(super) fn new(response: reqwest::blocking::Response) -> Self {
        Self {
            response,
            prefix: Vec::new(),
        }
    }

    pub(super) fn with_prefix(response: reqwest::blocking::Response, prefix: Vec<u8>) -> Self {
        Self { response, prefix }
    }
}

fn send_runtime_local_rewrite_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let route_kind = runtime_local_rewrite_route_kind(&request.path_and_query);
    let body = prepare_runtime_smart_context_http_body(
        request_id,
        request,
        &shared.runtime_shared,
        route_kind,
    )
    .into_owned();
    match &shared.provider {
        RuntimeLocalRewriteProviderOptions::Anthropic { auth } => {
            let auth_attempts = runtime_local_rewrite_anthropic_auth_attempts(shared, auth);
            if auth_attempts.is_empty() {
                anyhow::bail!("Anthropic provider has no auth configured");
            }
            let auth_attempt_count = auth_attempts.len();
            if path_without_query(&request.path_and_query).ends_with("/responses") {
                let model_selection = runtime_local_rewrite_model_selection(
                    shared,
                    RuntimeProviderBridgeKind::Anthropic,
                    request,
                    &body,
                    prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL,
                );
                let model_chain = runtime_provider_model_fallback_chain(
                    RuntimeProviderBridgeKind::Anthropic,
                    &model_selection.model,
                );
                let upstream_url = runtime_openai_standard_provider_upstream_url(
                    RuntimeProviderBridgeKind::Anthropic,
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                );
                for (auth_index, selected_auth) in auth_attempts.into_iter().enumerate() {
                    for (model_index, model) in model_chain.iter().enumerate() {
                        let model_body =
                            runtime_provider_request_body_with_model(&model_selection.body, model);
                        let translated = runtime_chat_compatible_request_body(
                            &model_body,
                            &shared.deepseek_conversations,
                            RuntimeProviderBridgeKind::Anthropic,
                            prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL,
                            false,
                        )?;
                        if let Ok(mut pending) = shared.deepseek_pending_messages.lock() {
                            pending.insert(request_id, translated.messages);
                        }
                        let send_result =
                            send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
                                RuntimeLocalRewriteSearchFallbackRequest {
                                    request_id,
                                    request,
                                    shared,
                                    upstream_url: &upstream_url,
                                    body: translated.body,
                                    provider_kind: RuntimeProviderBridgeKind::Anthropic,
                                    auth_label: selected_auth.label.as_str(),
                                    model,
                                    auth_factory: || RuntimeLocalRewritePreparedAuth::Anthropic {
                                        auth: &selected_auth.auth,
                                    },
                                },
                            )?;
                        let (status, parts, class) = match send_result {
                            RuntimeLocalRewritePreparedSendResult::Live(response) => {
                                return Ok(RuntimeLocalRewriteUpstreamResult {
                                    response: RuntimeLocalRewriteUpstreamResponse::Live(
                                        RuntimeLocalRewriteLiveResponse::new(response),
                                    ),
                                    gemini_context: None,
                                    copilot_context: None,
                                });
                            }
                            RuntimeLocalRewritePreparedSendResult::Error {
                                status,
                                parts,
                                class,
                            } => (status, parts, class),
                        };
                        if model_index + 1 < model_chain.len()
                            && runtime_provider_should_retry_with_next_model(class)
                        {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_provider_model_fallback",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field("provider", "anthropic"),
                                        runtime_proxy_log_field(
                                            "auth",
                                            selected_auth.label.as_str(),
                                        ),
                                        runtime_proxy_log_field("from_model", model.as_str()),
                                        runtime_proxy_log_field(
                                            "to_model",
                                            model_chain[model_index + 1].as_str(),
                                        ),
                                        runtime_proxy_log_field("status", status.to_string()),
                                        runtime_proxy_log_field("class", format!("{class:?}")),
                                    ],
                                ),
                            );
                            continue;
                        }
                        if auth_index + 1 < auth_attempt_count
                            && runtime_provider_should_rotate_auth_after_response(class)
                        {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_provider_auth_rotate",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field("provider", "anthropic"),
                                        runtime_proxy_log_field(
                                            "auth",
                                            selected_auth.label.as_str(),
                                        ),
                                        runtime_proxy_log_field("status", status.to_string()),
                                        runtime_proxy_log_field("class", format!("{class:?}")),
                                    ],
                                ),
                            );
                            break;
                        }
                        return Ok(RuntimeLocalRewriteUpstreamResult {
                            response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                            gemini_context: None,
                            copilot_context: None,
                        });
                    }
                    if auth_index + 1 < auth_attempt_count {
                        continue;
                    }
                }
                anyhow::bail!("no Anthropic model attempts were available");
            } else {
                let upstream_url = runtime_local_rewrite_upstream_url(
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                );
                for (auth_index, selected_auth) in auth_attempts.into_iter().enumerate() {
                    let response = send_runtime_local_rewrite_prepared_request(
                        request_id,
                        request,
                        shared,
                        &upstream_url,
                        body.clone(),
                        RuntimeLocalRewritePreparedAuth::Anthropic {
                            auth: &selected_auth.auth,
                        },
                    )?;
                    let status = response.status().as_u16();
                    if status >= 400 {
                        let parts =
                            runtime_local_rewrite_buffered_response_from_response(response)?;
                        let class = runtime_provider_error_class(
                            RuntimeProviderBridgeKind::Anthropic,
                            status,
                            &parts.body,
                        );
                        if auth_index + 1 < auth_attempt_count
                            && runtime_provider_should_rotate_auth_after_response(class)
                        {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_provider_auth_rotate",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field("provider", "anthropic"),
                                        runtime_proxy_log_field(
                                            "auth",
                                            selected_auth.label.as_str(),
                                        ),
                                        runtime_proxy_log_field("status", status.to_string()),
                                        runtime_proxy_log_field("class", format!("{class:?}")),
                                    ],
                                ),
                            );
                            continue;
                        }
                        return Ok(RuntimeLocalRewriteUpstreamResult {
                            response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                            gemini_context: None,
                            copilot_context: None,
                        });
                    }
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(
                            RuntimeLocalRewriteLiveResponse::new(response),
                        ),
                        gemini_context: None,
                        copilot_context: None,
                    });
                }
                anyhow::bail!("no Anthropic auth attempts were available")
            }
        }
        RuntimeLocalRewriteProviderOptions::Copilot { auth } => {
            send_runtime_copilot_upstream_request(request_id, request, shared, body, auth)
        }
        RuntimeLocalRewriteProviderOptions::OpenAiResponses { api_keys } => {
            let upstream_url = runtime_local_rewrite_upstream_url(
                &shared.upstream_base_url,
                &shared.mount_path,
                &request.path_and_query,
            );
            let body = if path_without_query(&request.path_and_query).ends_with("/responses") {
                runtime_local_rewrite_model_selection(
                    shared,
                    RuntimeProviderBridgeKind::OpenAiResponses,
                    request,
                    &body,
                    "",
                )
                .body
            } else {
                body
            };
            let auth_attempts = runtime_local_rewrite_api_key_attempts(shared, api_keys);
            let selected_api_key = auth_attempts.first().map(|(_, api_key)| *api_key);
            let response = send_runtime_local_rewrite_prepared_request(
                request_id,
                request,
                shared,
                &upstream_url,
                body,
                RuntimeLocalRewritePreparedAuth::OpenAiResponses {
                    api_key: selected_api_key,
                },
            )?;
            Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Live(
                    RuntimeLocalRewriteLiveResponse::new(response),
                ),
                gemini_context: None,
                copilot_context: None,
            })
        }
        RuntimeLocalRewriteProviderOptions::DeepSeek { api_keys } => {
            send_runtime_deepseek_upstream_request(request_id, request, shared, body, api_keys)
        }
        RuntimeLocalRewriteProviderOptions::Gemini { auth, .. } => {
            send_runtime_gemini_upstream_request(request_id, request, shared, body, auth)
        }
    }
}

fn runtime_local_rewrite_route_kind(path_and_query: &str) -> RuntimeRouteKind {
    let path = path_without_query(path_and_query);
    if path.ends_with("/responses") || path.ends_with("/chat/completions") {
        RuntimeRouteKind::Responses
    } else if path.ends_with("/responses/compact") {
        RuntimeRouteKind::Compact
    } else {
        runtime_proxy_request_lane(path_and_query, false)
    }
}
