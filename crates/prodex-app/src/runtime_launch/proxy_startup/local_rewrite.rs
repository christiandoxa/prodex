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
use super::local_rewrite_response::{
    respond_runtime_local_rewrite_proxy_request,
    runtime_local_rewrite_buffered_response_from_response,
};
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_chat_completions_upstream_url,
    runtime_local_rewrite_anthropic_auth_attempts, runtime_local_rewrite_upstream_url,
    send_runtime_local_rewrite_prepared_request,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_error_class, runtime_provider_model_fallback_chain,
    runtime_provider_model_from_body, runtime_provider_models_buffered_response,
    runtime_provider_request_body_with_model, runtime_provider_request_ledger_message,
    runtime_provider_should_retry_with_next_model,
    runtime_provider_should_rotate_auth_after_response,
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
    pub(super) api_key_cursor: Arc<AtomicUsize>,
    pub(super) client: reqwest::blocking::Client,
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
            let auth_attempts = runtime_local_rewrite_anthropic_auth_attempts(shared, auth);
            if auth_attempts.is_empty() {
                anyhow::bail!("Anthropic provider has no auth configured");
            }
            let auth_attempt_count = auth_attempts.len();
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
                for (auth_index, selected_auth) in auth_attempts.into_iter().enumerate() {
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
                            if model_index + 1 < model_chain.len()
                                && runtime_provider_should_retry_with_next_model(class)
                            {
                                runtime_proxy_log(
                                    &shared.runtime_shared,
                                    runtime_proxy_structured_log_message(
                                        "local_rewrite_provider_model_fallback",
                                        [
                                            runtime_proxy_log_field(
                                                "request",
                                                request_id.to_string(),
                                            ),
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
                                            runtime_proxy_log_field(
                                                "request",
                                                request_id.to_string(),
                                            ),
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
                        return Ok(RuntimeLocalRewriteUpstreamResult {
                            response: RuntimeLocalRewriteUpstreamResponse::Live(response),
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
                        response: RuntimeLocalRewriteUpstreamResponse::Live(response),
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
            send_runtime_deepseek_upstream_request(request_id, request, shared, body, api_keys)
        }
        RuntimeLocalRewriteProviderOptions::Gemini { auth } => {
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
