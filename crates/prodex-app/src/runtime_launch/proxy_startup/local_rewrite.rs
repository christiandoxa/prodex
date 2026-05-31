use super::deepseek_rewrite::*;
use super::gemini_rewrite::*;
use super::local_rewrite_copilot::{
    RuntimeCopilotOAuthPool, RuntimeCopilotRequestContext,
    runtime_copilot_oauth_pool_from_provider, send_runtime_copilot_upstream_request,
};
use super::local_rewrite_gemini::{
    RuntimeGeminiOAuthPool, RuntimeGeminiRequestContext, runtime_gemini_oauth_pool_from_provider,
    send_runtime_gemini_upstream_request,
};
use super::local_rewrite_response::{
    respond_runtime_local_rewrite_proxy_request,
    runtime_local_rewrite_buffered_response_from_response,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_error_class, runtime_provider_label,
    runtime_provider_model_fallback_chain, runtime_provider_model_from_body,
    runtime_provider_models_buffered_response, runtime_provider_request_body_with_model,
    runtime_provider_request_ledger_message, runtime_provider_should_retry_with_next_model,
};
use super::*;

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
    api_key_cursor: Arc<AtomicUsize>,
    client: reqwest::blocking::Client,
}

#[derive(Clone)]
pub(crate) enum RuntimeLocalRewriteProviderOptions {
    Anthropic { auth: RuntimeAnthropicProviderAuth },
    Copilot { auth: RuntimeCopilotProviderAuth },
    OpenAiResponses,
    DeepSeek { api_keys: Vec<String> },
    Gemini { auth: RuntimeGeminiProviderAuth },
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
            RuntimeLocalRewriteProviderOptions::OpenAiResponses => {
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
    } = options;
    let log_path = initialize_runtime_proxy_log_path();
    let server = Arc::new(
        TinyServer::http("127.0.0.1:0")
            .map_err(|err| anyhow::anyhow!("failed to bind runtime local rewrite proxy: {err}"))?,
    );
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
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime local rewrite proxy started listen_addr={listen_addr} smart_context_enabled={smart_context_enabled} presidio_redaction_enabled={presidio_redaction_enabled} upstream_base_url={upstream_base_url} upstream_proxy_mode={}",
            runtime_upstream_proxy_mode_label(true)
        ),
    );
    let gemini_oauth_pool = runtime_gemini_oauth_pool_from_provider(&provider);
    let copilot_oauth_pool = runtime_copilot_oauth_pool_from_provider(&provider);
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
        api_key_cursor: Arc::new(AtomicUsize::new(0)),
        client: build_runtime_local_rewrite_http_client()?,
    };
    if let Some(pool) = shared.gemini_oauth_pool.as_ref() {
        pool.spawn_quota_refresh(shared.runtime_shared.log_path.clone());
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    let mut worker_threads = Vec::new();
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
    let websocket = is_tiny_http_websocket_upgrade(&request);
    let request_transport = if websocket { "websocket" } else { "http" };
    let runtime_shared = &shared.runtime_shared;
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

    let request_id = runtime_proxy_next_request_id(runtime_shared);
    if websocket {
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
    if matches!(
        &shared.provider,
        RuntimeLocalRewriteProviderOptions::Anthropic { .. }
            | RuntimeLocalRewriteProviderOptions::DeepSeek { .. }
            | RuntimeLocalRewriteProviderOptions::Gemini { .. }
            | RuntimeLocalRewriteProviderOptions::Copilot { .. }
    ) && path_without_query(&captured.path_and_query).ends_with("/responses/compact")
    {
        let provider_name = match &shared.provider {
            RuntimeLocalRewriteProviderOptions::Anthropic { .. } => "Anthropic",
            RuntimeLocalRewriteProviderOptions::Gemini { .. } => "Gemini",
            RuntimeLocalRewriteProviderOptions::Copilot { .. } => "GitHub Copilot",
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
        let _ = request.respond(build_runtime_proxy_response_from_parts(parts));
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
}

pub(super) struct RuntimeLocalRewriteUpstreamResult {
    pub(super) response: RuntimeLocalRewriteUpstreamResponse,
    pub(super) gemini_context: Option<RuntimeGeminiRequestContext>,
    pub(super) copilot_context: Option<RuntimeCopilotRequestContext>,
}

pub(super) enum RuntimeLocalRewriteUpstreamResponse {
    Live(reqwest::blocking::Response),
    Buffered(RuntimeHeapTrimmedBufferedResponseParts),
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
            let auth = runtime_local_rewrite_next_anthropic_auth(shared, auth)
                .context("Anthropic provider has no auth configured")?;
            if path_without_query(&request.path_and_query).ends_with("/responses") {
                let requested_model = runtime_provider_model_from_body(&body)
                    .unwrap_or_else(|| prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL.to_string());
                let model_chain = runtime_provider_model_fallback_chain(
                    RuntimeProviderBridgeKind::Anthropic,
                    &requested_model,
                );
                let upstream_url = runtime_chat_completions_upstream_url(
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                );
                for (model_index, model) in model_chain.iter().enumerate() {
                    let model_body = runtime_provider_request_body_with_model(&body, model);
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
                    let response = send_runtime_local_rewrite_prepared_request(
                        request_id,
                        request,
                        shared,
                        &upstream_url,
                        translated.body,
                        RuntimeLocalRewritePreparedAuth::Anthropic { auth: &auth },
                    )?;
                    let status = response.status().as_u16();
                    if status >= 400 && model_index + 1 < model_chain.len() {
                        let parts =
                            runtime_local_rewrite_buffered_response_from_response(response)?;
                        let class = runtime_provider_error_class(
                            RuntimeProviderBridgeKind::Anthropic,
                            status,
                            &parts.body,
                        );
                        if runtime_provider_should_retry_with_next_model(class) {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_provider_model_fallback",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field("provider", "anthropic"),
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
                        return Ok(RuntimeLocalRewriteUpstreamResult {
                            response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                            gemini_context: None,
                            copilot_context: None,
                        });
                    }
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(response),
                        gemini_context: None,
                        copilot_context: None,
                    });
                }
                anyhow::bail!("no Anthropic model attempts were available");
            } else {
                let upstream_url = runtime_local_rewrite_upstream_url(
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                );
                let response = send_runtime_local_rewrite_prepared_request(
                    request_id,
                    request,
                    shared,
                    &upstream_url,
                    body,
                    RuntimeLocalRewritePreparedAuth::Anthropic { auth: &auth },
                )?;
                Ok(RuntimeLocalRewriteUpstreamResult {
                    response: RuntimeLocalRewriteUpstreamResponse::Live(response),
                    gemini_context: None,
                    copilot_context: None,
                })
            }
        }
        RuntimeLocalRewriteProviderOptions::Copilot { auth } => {
            send_runtime_copilot_upstream_request(request_id, request, shared, body, auth)
        }
        RuntimeLocalRewriteProviderOptions::OpenAiResponses => {
            let upstream_url = runtime_local_rewrite_upstream_url(
                &shared.upstream_base_url,
                &shared.mount_path,
                &request.path_and_query,
            );
            let response = send_runtime_local_rewrite_prepared_request(
                request_id,
                request,
                shared,
                &upstream_url,
                body,
                RuntimeLocalRewritePreparedAuth::OpenAiResponses,
            )?;
            Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Live(response),
                gemini_context: None,
                copilot_context: None,
            })
        }
        RuntimeLocalRewriteProviderOptions::DeepSeek { api_keys } => {
            let api_key = runtime_local_rewrite_next_api_key(shared, api_keys)
                .context("DeepSeek provider has no API keys configured")?;
            if path_without_query(&request.path_and_query).ends_with("/responses") {
                let requested_model = runtime_provider_model_from_body(&body)
                    .unwrap_or_else(|| prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL.to_string());
                let model_chain = runtime_provider_model_fallback_chain(
                    RuntimeProviderBridgeKind::DeepSeek,
                    &requested_model,
                );
                let upstream_url = runtime_deepseek_upstream_url(
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                );
                for (model_index, model) in model_chain.iter().enumerate() {
                    let model_body = runtime_provider_request_body_with_model(&body, model);
                    let translated = runtime_deepseek_chat_request_body(
                        &model_body,
                        &shared.deepseek_conversations,
                    )?;
                    if let Ok(mut pending) = shared.deepseek_pending_messages.lock() {
                        pending.insert(request_id, translated.messages);
                    }
                    let response = send_runtime_local_rewrite_prepared_request(
                        request_id,
                        request,
                        shared,
                        &upstream_url,
                        translated.body,
                        RuntimeLocalRewritePreparedAuth::DeepSeek { api_key },
                    )?;
                    let status = response.status().as_u16();
                    if status >= 400 && model_index + 1 < model_chain.len() {
                        let parts =
                            runtime_local_rewrite_buffered_response_from_response(response)?;
                        let class = runtime_provider_error_class(
                            RuntimeProviderBridgeKind::DeepSeek,
                            status,
                            &parts.body,
                        );
                        if runtime_provider_should_retry_with_next_model(class) {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_provider_model_fallback",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field("provider", "deepseek"),
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
                        return Ok(RuntimeLocalRewriteUpstreamResult {
                            response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                            gemini_context: None,
                            copilot_context: None,
                        });
                    }
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(response),
                        gemini_context: None,
                        copilot_context: None,
                    });
                }
                anyhow::bail!("no DeepSeek model attempts were available");
            } else {
                let upstream_url = runtime_deepseek_upstream_url(
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                );
                let response = send_runtime_local_rewrite_prepared_request(
                    request_id,
                    request,
                    shared,
                    &upstream_url,
                    body,
                    RuntimeLocalRewritePreparedAuth::DeepSeek { api_key },
                )?;
                Ok(RuntimeLocalRewriteUpstreamResult {
                    response: RuntimeLocalRewriteUpstreamResponse::Live(response),
                    gemini_context: None,
                    copilot_context: None,
                })
            }
        }
        RuntimeLocalRewriteProviderOptions::Gemini { auth } => {
            send_runtime_gemini_upstream_request(request_id, request, shared, body, auth)
        }
    }
}

pub(super) enum RuntimeLocalRewritePreparedAuth<'a> {
    Anthropic { auth: &'a RuntimeAnthropicAuth },
    Copilot { api_key: &'a str },
    OpenAiResponses,
    DeepSeek { api_key: &'a str },
    Gemini { auth: &'a RuntimeGeminiAuth },
}

impl RuntimeLocalRewritePreparedAuth<'_> {
    fn bridge_kind(&self) -> RuntimeProviderBridgeKind {
        match self {
            RuntimeLocalRewritePreparedAuth::Anthropic { .. } => {
                RuntimeProviderBridgeKind::Anthropic
            }
            RuntimeLocalRewritePreparedAuth::Copilot { .. } => RuntimeProviderBridgeKind::Copilot,
            RuntimeLocalRewritePreparedAuth::OpenAiResponses => {
                RuntimeProviderBridgeKind::OpenAiResponses
            }
            RuntimeLocalRewritePreparedAuth::DeepSeek { .. } => RuntimeProviderBridgeKind::DeepSeek,
            RuntimeLocalRewritePreparedAuth::Gemini { .. } => RuntimeProviderBridgeKind::Gemini,
        }
    }
}

pub(super) fn send_runtime_local_rewrite_prepared_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    upstream_url: &str,
    body: Vec<u8>,
    auth: RuntimeLocalRewritePreparedAuth<'_>,
) -> Result<reqwest::blocking::Response> {
    let provider_kind = auth.bridge_kind();
    let body_bytes = body.len();
    let model = runtime_provider_model_from_body(&body);
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime local rewrite",
            request.method
        )
    })?;
    let mut upstream_request = shared.client.request(method, upstream_url);
    match auth {
        RuntimeLocalRewritePreparedAuth::Anthropic { auth } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(
                    reqwest::header::ACCEPT,
                    "text/event-stream, application/json",
                );
            match auth {
                RuntimeAnthropicAuth::ApiKey { api_key } => {
                    upstream_request = upstream_request.bearer_auth(api_key);
                }
                RuntimeAnthropicAuth::OAuth { access_token } => {
                    upstream_request = upstream_request
                        .bearer_auth(access_token)
                        .header("anthropic-beta", "oauth-2025-04-20");
                }
            }
            if let Some(user_agent) = runtime_local_rewrite_header(request, "user-agent") {
                upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
            }
        }
        RuntimeLocalRewritePreparedAuth::Copilot { api_key } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(
                    reqwest::header::ACCEPT,
                    "text/event-stream, application/json",
                )
                .bearer_auth(api_key)
                .header("copilot-integration-id", "vscode-chat")
                .header("editor-version", "vscode/1.95.0")
                .header("editor-plugin-version", "copilot-chat/0.26.7")
                .header("openai-intent", "conversation-panel")
                .header("x-github-api-version", "2025-04-01")
                .header("x-request-id", format!("prodex-{request_id}"))
                .header("x-vscode-user-agent-library-version", "electron-fetch")
                .header("X-Initiator", runtime_copilot_initiator_header(request));
            if let Some(user_agent) = runtime_local_rewrite_header(request, "user-agent") {
                upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
            } else {
                upstream_request = upstream_request
                    .header(reqwest::header::USER_AGENT, "GitHubCopilotChat/0.26.7");
            }
            if runtime_copilot_request_has_vision_input(&body) {
                upstream_request = upstream_request.header("copilot-vision-request", "true");
            }
        }
        RuntimeLocalRewritePreparedAuth::OpenAiResponses => {
            for (name, value) in &request.headers {
                if should_skip_runtime_local_rewrite_request_header(name) {
                    continue;
                }
                upstream_request = upstream_request.header(name.as_str(), value.as_str());
            }
        }
        RuntimeLocalRewritePreparedAuth::DeepSeek { api_key } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .bearer_auth(api_key);
            if let Some(user_agent) = runtime_local_rewrite_header(request, "user-agent") {
                upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
            }
        }
        RuntimeLocalRewritePreparedAuth::Gemini { auth } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(
                    reqwest::header::ACCEPT,
                    "text/event-stream, application/json",
                );
            match auth {
                RuntimeGeminiAuth::ApiKey { api_key } => {
                    upstream_request = upstream_request.header("x-goog-api-key", api_key);
                }
                RuntimeGeminiAuth::OAuth { access_token, .. } => {
                    upstream_request = upstream_request.bearer_auth(access_token);
                }
            }
            if let Some(user_agent) = runtime_local_rewrite_header(request, "user-agent") {
                upstream_request = upstream_request.header(reqwest::header::USER_AGENT, user_agent);
            }
        }
    }
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_upstream_start",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("method", request.method.as_str()),
                runtime_proxy_log_field("url", upstream_url),
                runtime_proxy_log_field("provider", runtime_provider_label(provider_kind)),
                runtime_proxy_log_field("model", model.as_deref().unwrap_or("unknown")),
                runtime_proxy_log_field("body_bytes", body_bytes.to_string()),
            ],
        ),
    );
    let started_at = Instant::now();
    let response = upstream_request
        .body(body)
        .send()
        .with_context(|| format!("failed to proxy local provider request to {upstream_url}"))?;
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_upstream_response",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("status", response.status().as_u16().to_string()),
                runtime_proxy_log_field("elapsed_ms", started_at.elapsed().as_millis().to_string()),
            ],
        ),
    );
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_provider_request_ledger_message(
            request_id,
            provider_kind,
            &request.path_and_query,
            model.as_deref(),
            response.status().as_u16(),
            started_at.elapsed().as_millis(),
            body_bytes,
        ),
    );
    Ok(response)
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

pub(super) fn runtime_local_rewrite_upstream_url(
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let base_url = base_url.trim_end_matches('/');
    let mount_path = mount_path.trim_end_matches('/');
    let (path, query) = path_and_query
        .split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((path_and_query, None));
    let suffix = path
        .strip_prefix(mount_path)
        .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
        .unwrap_or(path);
    let mut upstream_url = if suffix.is_empty() {
        base_url.to_string()
    } else if suffix.starts_with('/') {
        format!("{base_url}{suffix}")
    } else {
        format!("{base_url}/{suffix}")
    };
    if let Some(query) = query {
        upstream_url.push('?');
        upstream_url.push_str(query);
    }
    upstream_url
}

fn runtime_deepseek_upstream_url(base_url: &str, mount_path: &str, path_and_query: &str) -> String {
    let path = path_without_query(path_and_query);
    if path.ends_with("/responses") {
        return runtime_chat_completions_upstream_url(base_url, mount_path, path_and_query);
    }
    runtime_local_rewrite_upstream_url(base_url, mount_path, path_and_query)
}

fn runtime_chat_completions_upstream_url(
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let path = path_without_query(path_and_query);
    if path.ends_with("/responses") {
        return runtime_local_rewrite_upstream_url(base_url, mount_path, "/chat/completions");
    }
    runtime_local_rewrite_upstream_url(base_url, mount_path, path_and_query)
}

fn runtime_local_rewrite_next_api_key<'a>(
    shared: &RuntimeLocalRewriteProxyShared,
    api_keys: &'a [String],
) -> Option<&'a str> {
    if api_keys.is_empty() {
        return None;
    }
    if api_keys.len() == 1 {
        return Some(api_keys[0].as_str());
    }
    let index = shared.api_key_cursor.fetch_add(1, Ordering::Relaxed) % api_keys.len();
    Some(api_keys[index].as_str())
}

fn runtime_local_rewrite_next_anthropic_auth(
    shared: &RuntimeLocalRewriteProxyShared,
    auth: &RuntimeAnthropicProviderAuth,
) -> Option<RuntimeAnthropicAuth> {
    match auth {
        RuntimeAnthropicProviderAuth::ApiKeys { api_keys } => {
            runtime_local_rewrite_next_api_key(shared, api_keys).map(|api_key| {
                RuntimeAnthropicAuth::ApiKey {
                    api_key: api_key.to_string(),
                }
            })
        }
        RuntimeAnthropicProviderAuth::OAuthProfiles { profiles } => {
            if profiles.is_empty() {
                return None;
            }
            if profiles.len() == 1 {
                return Some(profiles[0].auth());
            }
            let index = shared.api_key_cursor.fetch_add(1, Ordering::Relaxed) % profiles.len();
            Some(profiles[index].auth())
        }
    }
}

pub(super) fn runtime_copilot_request_body_with_canonical_model(body: &[u8]) -> Vec<u8> {
    let Some(model) = runtime_provider_model_from_body(body) else {
        return body.to_vec();
    };
    let canonical =
        runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Copilot, &model)
            .into_iter()
            .next()
            .unwrap_or(model);
    runtime_provider_request_body_with_model(body, &canonical)
}

fn runtime_copilot_initiator_header(request: &RuntimeProxyRequest) -> &'static str {
    if runtime_copilot_request_has_agent_input(&request.body) {
        "agent"
    } else {
        "user"
    }
}

fn runtime_copilot_request_has_agent_input(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.as_object().is_some_and(|object| {
                    object
                        .get("role")
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|role| !role.is_empty())
                        .is_none_or(|role| role.eq_ignore_ascii_case("assistant"))
                })
            })
        })
}

fn runtime_copilot_request_has_vision_input(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    runtime_copilot_value_contains_text(&value, "input_image")
        || runtime_copilot_value_contains_key(&value, "image_url")
}

fn runtime_copilot_value_contains_text(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains(needle),
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| runtime_copilot_value_contains_text(value, needle)),
        serde_json::Value::Object(object) => object
            .values()
            .any(|value| runtime_copilot_value_contains_text(value, needle)),
        _ => false,
    }
}

fn runtime_copilot_value_contains_key(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| runtime_copilot_value_contains_key(value, needle)),
        serde_json::Value::Object(object) => {
            object.contains_key(needle)
                || object
                    .values()
                    .any(|value| runtime_copilot_value_contains_key(value, needle))
        }
        _ => false,
    }
}

fn runtime_local_rewrite_header<'a>(
    request: &'a RuntimeProxyRequest,
    expected_name: &str,
) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(expected_name))
        .map(|(_, value)| value.as_str())
}

fn should_skip_runtime_local_rewrite_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "connection"
            | "content-length"
            | "host"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    ) || lower.starts_with("sec-websocket-")
        || lower.starts_with("x-prodex-internal-")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deepseek_conversation_store() -> RuntimeDeepSeekConversationStore {
        Arc::new(Mutex::new(BTreeMap::new()))
    }

    #[test]
    fn deepseek_request_translation_maps_responses_input_and_tools() {
        let conversations = deepseek_conversation_store();
        let body = serde_json::json!({
            "model": "deepseek-v4-pro",
            "stream": true,
            "instructions": "Be concise.",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "List files"}]
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "README.md"
                }
            ],
            "tools": [
                {
                    "type": "function",
                    "name": "shell",
                    "description": "Run shell",
                    "parameters": {"type": "object"}
                },
                {"type": "web_search_preview"}
            ],
            "tool_choice": {
                "type": "function",
                "name": "shell"
            },
            "reasoning": {
                "effort": "xhigh"
            },
            "max_output_tokens": 123
        });

        let translated =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect("request should translate");
        let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(translated["model"], "deepseek-v4-pro");
        assert_eq!(translated["stream"], true);
        assert_eq!(translated["max_tokens"], 123);
        assert_eq!(translated["messages"][0]["role"], "system");
        assert_eq!(translated["messages"][1]["content"], "List files");
        assert_eq!(translated["messages"].as_array().unwrap().len(), 2);
        assert_eq!(translated["tools"].as_array().unwrap().len(), 1);
        assert_eq!(translated["tools"][0]["function"]["name"], "shell");
        assert!(translated.get("tool_choice").is_none());
        assert_eq!(translated["thinking"]["type"], "enabled");
        assert_eq!(translated["reasoning_effort"], "max");
    }

    #[test]
    fn deepseek_request_translation_prepends_previous_response_history() {
        let conversations = deepseek_conversation_store();
        conversations.lock().unwrap().insert(
            "resp_prev".to_string(),
            vec![
                serde_json::json!({"role": "user", "content": "old prompt"}),
                serde_json::json!({"role": "assistant", "content": "old answer"}),
            ],
        );
        let body = serde_json::json!({
            "model": "deepseek-v4-pro",
            "previous_response_id": "resp_prev",
            "input": "new prompt"
        });

        let translated =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect("request should translate");
        let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(translated["messages"][0]["content"], "old prompt");
        assert_eq!(translated["messages"][1]["content"], "old answer");
        assert_eq!(translated["messages"][2]["content"], "new prompt");
    }

    #[test]
    fn deepseek_sse_reader_maps_text_and_tool_calls_to_responses_events() {
        let conversations = deepseek_conversation_store();
        let stream = concat!(
            "data: {\"id\":\"chatcmpl_1\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"shell\",\"arguments\":\"{\\\"cmd\\\":\"}}]}}]}\n\n",
            "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"ls\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let mut reader = RuntimeDeepSeekChatSseReader::new(
            Cursor::new(stream.as_bytes()),
            7,
            Vec::new(),
            conversations,
        );
        let mut output = String::new();
        reader.read_to_string(&mut output).unwrap();

        assert!(output.contains("event: response.created"));
        assert!(output.contains("\"type\":\"response.output_text.delta\""));
        assert!(output.contains("\"delta\":\"hi\""));
        assert!(output.contains("\"type\":\"response.output_item.added\""));
        assert!(output.contains("\"type\":\"response.function_call_arguments.delta\""));
        assert!(output.contains("\"type\":\"response.output_item.done\""));
        assert!(output.contains("\"arguments\":\"{\\\"cmd\\\":\\\"rtk ls\\\"}\""));
        assert!(output.contains("event: response.completed"));
    }
}
