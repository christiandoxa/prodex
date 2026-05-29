use super::deepseek_rewrite::*;
use super::gemini_rewrite::*;
use super::gemini_sse::{RuntimeGeminiBindingRecorder, RuntimeGeminiGenerateSseReader};
use super::*;

pub(crate) const RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH: &str = "/v1";
const RUNTIME_LOCAL_REWRITE_PROFILE: &str = "local";
const RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT: usize = 4096;

#[derive(Clone)]
struct RuntimeLocalRewriteProxyShared {
    runtime_shared: RuntimeRotationProxyShared,
    upstream_base_url: String,
    mount_path: String,
    provider: RuntimeLocalRewriteProviderOptions,
    deepseek_conversations: RuntimeDeepSeekConversationStore,
    deepseek_pending_messages: RuntimeDeepSeekPendingMessages,
    gemini_conversations: RuntimeDeepSeekConversationStore,
    gemini_oauth_pool: Option<RuntimeGeminiOAuthPool>,
    client: reqwest::blocking::Client,
}

#[derive(Clone)]
pub(crate) enum RuntimeLocalRewriteProviderOptions {
    OpenAiResponses,
    DeepSeek { api_key: String },
    Gemini { auth: RuntimeGeminiProviderAuth },
}

#[derive(Clone)]
struct RuntimeGeminiOAuthPool {
    state: Arc<Mutex<RuntimeGeminiOAuthPoolState>>,
}

#[derive(Debug)]
struct RuntimeGeminiOAuthPoolState {
    profiles: Vec<RuntimeGeminiOAuthProfileAuth>,
    next_index: usize,
    response_profile_bindings: BTreeMap<String, String>,
    tool_call_profile_bindings: BTreeMap<String, String>,
}

#[derive(Clone)]
struct RuntimeGeminiSelectedAuth {
    profile_name: String,
    auth: RuntimeGeminiAuth,
    hard_affinity: bool,
}

#[derive(Clone)]
struct RuntimeGeminiRequestContext {
    profile_name: String,
    conversation_messages: Vec<serde_json::Value>,
    binding_recorder: Option<RuntimeGeminiBindingRecorder>,
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
    let shared = RuntimeLocalRewriteProxyShared {
        runtime_shared: runtime_shared.clone(),
        upstream_base_url,
        mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        provider,
        deepseek_conversations: Arc::new(Mutex::new(BTreeMap::new())),
        deepseek_pending_messages: Arc::new(Mutex::new(BTreeMap::new())),
        gemini_conversations: Arc::new(Mutex::new(BTreeMap::new())),
        gemini_oauth_pool,
        client: build_runtime_local_rewrite_http_client()?,
    };
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

fn runtime_gemini_oauth_pool_from_provider(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Option<RuntimeGeminiOAuthPool> {
    let RuntimeLocalRewriteProviderOptions::Gemini {
        auth: RuntimeGeminiProviderAuth::OAuthProfiles { profiles },
    } = provider
    else {
        return None;
    };
    Some(RuntimeGeminiOAuthPool {
        state: Arc::new(Mutex::new(RuntimeGeminiOAuthPoolState {
            profiles: profiles.clone(),
            next_index: 0,
            response_profile_bindings: BTreeMap::new(),
            tool_call_profile_bindings: BTreeMap::new(),
        })),
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
        RuntimeLocalRewriteProviderOptions::DeepSeek { .. }
            | RuntimeLocalRewriteProviderOptions::Gemini { .. }
    ) && path_without_query(&captured.path_and_query).ends_with("/responses/compact")
    {
        let provider_name = match &shared.provider {
            RuntimeLocalRewriteProviderOptions::Gemini { .. } => "Gemini",
            _ => "DeepSeek",
        };
        let _ = request.respond(build_runtime_proxy_text_response(
            501,
            &format!("{provider_name} provider does not support Codex remote compact yet"),
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
}

struct RuntimeLocalRewriteUpstreamResult {
    response: RuntimeLocalRewriteUpstreamResponse,
    gemini_context: Option<RuntimeGeminiRequestContext>,
}

enum RuntimeLocalRewriteUpstreamResponse {
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
            })
        }
        RuntimeLocalRewriteProviderOptions::DeepSeek { api_key } => {
            let body = if path_without_query(&request.path_and_query).ends_with("/responses") {
                let translated =
                    runtime_deepseek_chat_request_body(&body, &shared.deepseek_conversations)?;
                if let Ok(mut pending) = shared.deepseek_pending_messages.lock() {
                    pending.insert(request_id, translated.messages);
                }
                translated.body
            } else {
                body
            };
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
            })
        }
        RuntimeLocalRewriteProviderOptions::Gemini { auth } => {
            send_runtime_gemini_upstream_request(request_id, request, shared, body, auth)
        }
    }
}

enum RuntimeLocalRewritePreparedAuth<'a> {
    OpenAiResponses,
    DeepSeek { api_key: &'a str },
    Gemini { auth: &'a RuntimeGeminiAuth },
}

fn send_runtime_local_rewrite_prepared_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    upstream_url: &str,
    body: Vec<u8>,
    auth: RuntimeLocalRewritePreparedAuth<'_>,
) -> Result<reqwest::blocking::Response> {
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime local rewrite",
            request.method
        )
    })?;
    let mut upstream_request = shared.client.request(method, upstream_url);
    match auth {
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
                runtime_proxy_log_field("body_bytes", body.len().to_string()),
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
    Ok(response)
}

fn send_runtime_gemini_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeGeminiProviderAuth,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let responses_route = path_without_query(&request.path_and_query).ends_with("/responses");
    let attempts = runtime_gemini_auth_attempts(auth, shared.gemini_oauth_pool.as_ref(), &body)?;
    for (attempt_index, selected) in attempts.iter().enumerate() {
        let translated = if responses_route {
            runtime_gemini_generate_request_body(
                &body,
                &shared.gemini_conversations,
                matches!(selected.auth, RuntimeGeminiAuth::OAuth { .. }),
                runtime_gemini_project_id(&selected.auth),
            )?
        } else {
            RuntimeGeminiTranslatedRequest {
                body: body.clone(),
                messages: Vec::new(),
                model: prodex_cli::SUPER_GEMINI_DEFAULT_MODEL.to_string(),
                stream: false,
            }
        };
        let upstream_url = runtime_gemini_upstream_url(
            &shared.upstream_base_url,
            &selected.auth,
            &translated.model,
            translated.stream,
        );
        let response = send_runtime_local_rewrite_prepared_request(
            request_id,
            request,
            shared,
            &upstream_url,
            translated.body,
            RuntimeLocalRewritePreparedAuth::Gemini {
                auth: &selected.auth,
            },
        )?;
        let status = response.status().as_u16();
        if runtime_gemini_should_rotate_after_quota_response(
            status,
            selected.hard_affinity,
            attempt_index,
            attempts.len(),
        ) {
            let parts = runtime_local_rewrite_buffered_response_from_response(response)?;
            if runtime_gemini_buffered_parts_are_quota_blocked(status, &parts) {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_gemini_quota_rotate",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("profile", selected.profile_name.as_str()),
                            runtime_proxy_log_field("status", status.to_string()),
                        ],
                    ),
                );
                continue;
            }
            return Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                gemini_context: None,
            });
        }

        let binding_recorder = shared
            .gemini_oauth_pool
            .as_ref()
            .map(|pool| runtime_gemini_binding_recorder(pool, selected.profile_name.clone()));
        let gemini_context = responses_route.then(|| RuntimeGeminiRequestContext {
            profile_name: selected.profile_name.clone(),
            conversation_messages: translated.messages,
            binding_recorder,
        });
        return Ok(RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Live(response),
            gemini_context,
        });
    }

    bail!("no Gemini auth attempts were available")
}

fn runtime_gemini_auth_attempts(
    auth: &RuntimeGeminiProviderAuth,
    pool: Option<&RuntimeGeminiOAuthPool>,
    body: &[u8],
) -> Result<Vec<RuntimeGeminiSelectedAuth>> {
    match auth {
        RuntimeGeminiProviderAuth::ApiKey { api_key } => Ok(vec![RuntimeGeminiSelectedAuth {
            profile_name: "api-key".to_string(),
            auth: RuntimeGeminiAuth::ApiKey {
                api_key: api_key.clone(),
            },
            hard_affinity: true,
        }]),
        RuntimeGeminiProviderAuth::OAuthProfiles { profiles } => {
            let pool = pool.context("Gemini OAuth pool was not initialized")?;
            pool.select_attempts(body, profiles)
        }
    }
}

impl RuntimeGeminiOAuthPool {
    fn select_attempts(
        &self,
        body: &[u8],
        fallback_profiles: &[RuntimeGeminiOAuthProfileAuth],
    ) -> Result<Vec<RuntimeGeminiSelectedAuth>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Gemini OAuth pool lock poisoned"))?;
        if let Some(profile_name) = state.affinity_profile_for_body(body)
            && let Some(profile) = state.profile_by_name(&profile_name)
        {
            return Ok(vec![RuntimeGeminiSelectedAuth {
                profile_name,
                auth: profile.auth(),
                hard_affinity: true,
            }]);
        }
        let profiles = if state.profiles.is_empty() {
            fallback_profiles.to_vec()
        } else {
            state.profiles.clone()
        };
        if profiles.is_empty() {
            bail!("Gemini OAuth pool is empty");
        }
        let start = state.next_index.min(profiles.len().saturating_sub(1));
        state.next_index = (start + 1) % profiles.len();
        Ok((0..profiles.len())
            .map(|offset| {
                let profile = profiles[(start + offset) % profiles.len()].clone();
                RuntimeGeminiSelectedAuth {
                    profile_name: profile.profile_name.clone(),
                    auth: profile.auth(),
                    hard_affinity: false,
                }
            })
            .collect())
    }
}

impl RuntimeGeminiOAuthPoolState {
    fn profile_by_name(&self, profile_name: &str) -> Option<RuntimeGeminiOAuthProfileAuth> {
        self.profiles
            .iter()
            .find(|profile| profile.profile_name == profile_name)
            .cloned()
    }

    fn affinity_profile_for_body(&self, body: &[u8]) -> Option<String> {
        let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
        if let Some(previous_response_id) = value
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str)
            && let Some(profile_name) = self.response_profile_bindings.get(previous_response_id)
        {
            return Some(profile_name.clone());
        }
        runtime_gemini_tool_output_call_ids_from_request(&value)
            .into_iter()
            .find_map(|call_id| self.tool_call_profile_bindings.get(&call_id).cloned())
    }

    fn remember_bindings(
        &mut self,
        profile_name: &str,
        response_id: &str,
        tool_call_ids: &[String],
    ) {
        if !response_id.trim().is_empty() {
            self.response_profile_bindings
                .insert(response_id.to_string(), profile_name.to_string());
        }
        for call_id in tool_call_ids {
            if !call_id.trim().is_empty() {
                self.tool_call_profile_bindings
                    .insert(call_id.clone(), profile_name.to_string());
            }
        }
        runtime_gemini_prune_binding_map(
            &mut self.response_profile_bindings,
            RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT,
        );
        runtime_gemini_prune_binding_map(
            &mut self.tool_call_profile_bindings,
            RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT,
        );
    }
}

fn runtime_gemini_binding_recorder(
    pool: &RuntimeGeminiOAuthPool,
    profile_name: String,
) -> RuntimeGeminiBindingRecorder {
    let pool = pool.clone();
    Arc::new(move |response_id, tool_call_ids| {
        if let Ok(mut state) = pool.state.lock() {
            state.remember_bindings(&profile_name, &response_id, &tool_call_ids);
        }
    })
}

fn runtime_gemini_prune_binding_map(map: &mut BTreeMap<String, String>, limit: usize) {
    while map.len() > limit {
        let Some(key) = map.keys().next().cloned() else {
            break;
        };
        map.remove(&key);
    }
}

fn runtime_gemini_tool_output_call_ids_from_request(value: &serde_json::Value) -> Vec<String> {
    value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_object)
        .filter(|object| {
            matches!(
                object.get("type").and_then(serde_json::Value::as_str),
                Some("function_call_output" | "mcp_call_output" | "mcp_tool_result")
            )
        })
        .filter_map(|object| {
            ["call_id", "tool_call_id", "id"]
                .into_iter()
                .find_map(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
        })
        .filter(|call_id| !call_id.trim().is_empty())
        .collect()
}

fn runtime_gemini_response_retryable_quota(status: u16) -> bool {
    matches!(status, 403 | 429)
}

fn runtime_gemini_should_rotate_after_quota_response(
    status: u16,
    hard_affinity: bool,
    attempt_index: usize,
    attempt_count: usize,
) -> bool {
    runtime_gemini_response_retryable_quota(status)
        && !hard_affinity
        && attempt_index + 1 < attempt_count
}

fn runtime_gemini_buffered_parts_are_quota_blocked(
    status: u16,
    parts: &RuntimeHeapTrimmedBufferedResponseParts,
) -> bool {
    runtime_gemini_response_retryable_quota(status)
        && (extract_runtime_proxy_quota_message(&parts.body).is_some()
            || runtime_gemini_google_quota_message(&parts.body).is_some())
}

fn runtime_gemini_google_quota_message(body: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    runtime_gemini_google_quota_message_from_value(&value)
}

fn runtime_gemini_google_quota_message_from_value(value: &serde_json::Value) -> Option<String> {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                let message = object
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| object.get("detail").and_then(serde_json::Value::as_str))
                    .or_else(|| object.get("error").and_then(serde_json::Value::as_str));
                let explicit_quota = ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(runtime_gemini_google_quota_code)
                });
                if explicit_quota {
                    return Some(
                        message
                            .unwrap_or("Gemini account quota was exhausted.")
                            .to_string(),
                    );
                }
                stack.extend(object.values());
            }
            serde_json::Value::Array(values) => {
                stack.extend(values);
            }
            _ => {}
        }
    }
    None
}

fn runtime_gemini_google_quota_code(code: &str) -> bool {
    matches!(
        code.trim().to_ascii_lowercase().as_str(),
        "resource_exhausted"
            | "quota_exceeded"
            | "rate_limit_exceeded"
            | "rate_limit_exceeded_error"
    )
}

fn runtime_gemini_remember_bindings_from_responses_body(
    recorder: Option<&RuntimeGeminiBindingRecorder>,
    body: &[u8],
) {
    let Some(recorder) = recorder else {
        return;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return;
    };
    let response_id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let tool_call_ids = runtime_gemini_tool_call_ids_from_responses_value(&value);
    if !response_id.trim().is_empty() || !tool_call_ids.is_empty() {
        recorder(response_id, tool_call_ids);
    }
}

fn runtime_gemini_tool_call_ids_from_responses_value(value: &serde_json::Value) -> Vec<String> {
    value
        .get("output")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_object)
        .filter(|object| {
            object
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|kind| kind == "function_call")
        })
        .filter_map(|object| {
            object
                .get("call_id")
                .or_else(|| object.get("id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .filter(|call_id| !call_id.trim().is_empty())
        .collect()
}

fn runtime_local_rewrite_buffered_response_from_response(
    response: reqwest::blocking::Response,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let status = response.status().as_u16();
    let headers = runtime_proxy_crate::runtime_forward_binary_response_headers(
        response
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_bytes())),
    );
    runtime_local_rewrite_buffered_response_parts(status, headers, response)
}

fn respond_runtime_local_rewrite_proxy_request(
    request_id: u64,
    request: tiny_http::Request,
    response: RuntimeLocalRewriteUpstreamResult,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let RuntimeLocalRewriteUpstreamResult {
        response,
        gemini_context,
    } = response;
    let response = match response {
        RuntimeLocalRewriteUpstreamResponse::Live(response) => response,
        RuntimeLocalRewriteUpstreamResponse::Buffered(parts) => {
            let _ = request.respond(build_runtime_proxy_response_from_parts(parts));
            return;
        }
    };
    let status = response.status().as_u16();
    let headers = runtime_proxy_crate::runtime_forward_binary_response_headers(
        response
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_bytes())),
    );
    let text_headers = runtime_proxy_crate::runtime_forward_text_response_headers(
        response
            .headers()
            .iter()
            .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.as_str(), value))),
    );
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let deepseek_responses_route = matches!(
        &shared.provider,
        RuntimeLocalRewriteProviderOptions::DeepSeek { .. }
    ) && path_without_query(&captured.path_and_query)
        .ends_with("/responses");
    let gemini_responses_route = matches!(
        &shared.provider,
        RuntimeLocalRewriteProviderOptions::Gemini { .. }
    ) && path_without_query(&captured.path_and_query)
        .ends_with("/responses");
    if deepseek_responses_route && (200..300).contains(&status) {
        let conversation_messages =
            runtime_deepseek_take_pending_messages(&shared.deepseek_pending_messages, request_id);
        if content_type.contains("text/event-stream") {
            let writer = request.into_writer();
            let streaming = RuntimeStreamingResponse {
                status,
                headers: vec![(
                    "content-type".to_string(),
                    "text/event-stream; charset=utf-8".to_string(),
                )],
                body: Box::new(RuntimeDeepSeekChatSseReader::new(
                    response,
                    request_id,
                    conversation_messages,
                    Arc::clone(&shared.deepseek_conversations),
                )),
                request_id,
                profile_name: RUNTIME_LOCAL_REWRITE_PROFILE.to_string(),
                log_path: shared.runtime_shared.log_path.clone(),
                shared: shared.runtime_shared.clone(),
                _inflight_guard: None,
            };
            let _ = write_runtime_streaming_response(writer, streaming);
            return;
        }

        let response = runtime_deepseek_chat_buffered_response_parts(
            status,
            response,
            request_id,
            conversation_messages,
            &shared.deepseek_conversations,
        )
        .map(build_runtime_proxy_response_from_parts)
        .unwrap_or_else(|err| build_runtime_proxy_text_response(502, &err.to_string()));
        let _ = request.respond(response);
        return;
    }

    if gemini_responses_route && (200..300).contains(&status) {
        let RuntimeGeminiRequestContext {
            profile_name,
            conversation_messages,
            binding_recorder,
        } = gemini_context.unwrap_or_else(|| RuntimeGeminiRequestContext {
            profile_name: RUNTIME_LOCAL_REWRITE_PROFILE.to_string(),
            conversation_messages: Vec::new(),
            binding_recorder: None,
        });
        if content_type.contains("text/event-stream") {
            let writer = request.into_writer();
            let streaming = RuntimeStreamingResponse {
                status,
                headers: vec![(
                    "content-type".to_string(),
                    "text/event-stream; charset=utf-8".to_string(),
                )],
                body: Box::new(RuntimeGeminiGenerateSseReader::new(
                    response,
                    request_id,
                    conversation_messages,
                    Arc::clone(&shared.gemini_conversations),
                    binding_recorder,
                )),
                request_id,
                profile_name,
                log_path: shared.runtime_shared.log_path.clone(),
                shared: shared.runtime_shared.clone(),
                _inflight_guard: None,
            };
            let _ = write_runtime_streaming_response(writer, streaming);
            return;
        }

        let response = runtime_gemini_generate_buffered_response_parts(
            status,
            response,
            request_id,
            conversation_messages,
            &shared.gemini_conversations,
        )
        .map(|parts| {
            runtime_gemini_remember_bindings_from_responses_body(
                binding_recorder.as_ref(),
                &parts.body,
            );
            build_runtime_proxy_response_from_parts(parts)
        })
        .unwrap_or_else(|err| build_runtime_proxy_text_response(502, &err.to_string()));
        let _ = request.respond(response);
        return;
    }

    if content_type.contains("text/event-stream") {
        let writer = request.into_writer();
        let streaming = RuntimeStreamingResponse {
            status,
            headers: text_headers,
            body: Box::new(response),
            request_id,
            profile_name: RUNTIME_LOCAL_REWRITE_PROFILE.to_string(),
            log_path: shared.runtime_shared.log_path.clone(),
            shared: shared.runtime_shared.clone(),
            _inflight_guard: None,
        };
        let _ = write_runtime_streaming_response(writer, streaming);
        return;
    }

    let response = runtime_local_rewrite_buffered_response_parts(status, headers, response)
        .map(build_runtime_proxy_response_from_parts)
        .unwrap_or_else(|err| build_runtime_proxy_text_response(502, &err.to_string()));
    let _ = request.respond(response);
}

fn runtime_local_rewrite_buffered_response_parts(
    status: u16,
    headers: Vec<(String, Vec<u8>)>,
    mut response: reqwest::blocking::Response,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let mut body = Vec::new();
    response
        .read_to_end(&mut body)
        .context("failed to read local provider response body")?;
    Ok(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers,
        body: body.into(),
    })
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

fn runtime_local_rewrite_upstream_url(
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
        return runtime_local_rewrite_upstream_url(base_url, mount_path, "/chat/completions");
    }
    runtime_local_rewrite_upstream_url(base_url, mount_path, path_and_query)
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

    fn gemini_profile(profile_name: &str) -> RuntimeGeminiOAuthProfileAuth {
        RuntimeGeminiOAuthProfileAuth {
            profile_name: profile_name.to_string(),
            access_token: format!("token-{profile_name}"),
            project_id: Some(format!("project-{profile_name}")),
        }
    }

    fn gemini_pool(profile_names: &[&str]) -> RuntimeGeminiOAuthPool {
        RuntimeGeminiOAuthPool {
            state: Arc::new(Mutex::new(RuntimeGeminiOAuthPoolState {
                profiles: profile_names
                    .iter()
                    .map(|profile_name| gemini_profile(profile_name))
                    .collect(),
                next_index: 0,
                response_profile_bindings: BTreeMap::new(),
                tool_call_profile_bindings: BTreeMap::new(),
            })),
        }
    }

    #[test]
    fn gemini_oauth_pool_rotates_fresh_requests() {
        let pool = gemini_pool(&["alpha", "beta"]);
        let body = serde_json::to_vec(&serde_json::json!({"input": "hi"})).unwrap();

        let first = pool.select_attempts(&body, &[]).unwrap();
        let second = pool.select_attempts(&body, &[]).unwrap();

        assert_eq!(first[0].profile_name, "alpha");
        assert_eq!(first[1].profile_name, "beta");
        assert!(!first[0].hard_affinity);
        assert_eq!(second[0].profile_name, "beta");
        assert_eq!(second[1].profile_name, "alpha");
    }

    #[test]
    fn gemini_oauth_pool_preserves_previous_response_affinity() {
        let pool = gemini_pool(&["alpha", "beta"]);
        pool.state
            .lock()
            .unwrap()
            .remember_bindings("beta", "resp_1", &[]);
        let body =
            serde_json::to_vec(&serde_json::json!({"previous_response_id": "resp_1"})).unwrap();

        let attempts = pool.select_attempts(&body, &[]).unwrap();

        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].profile_name, "beta");
        assert!(attempts[0].hard_affinity);
    }

    #[test]
    fn gemini_oauth_pool_preserves_tool_output_affinity() {
        let pool = gemini_pool(&["alpha", "beta"]);
        pool.state
            .lock()
            .unwrap()
            .remember_bindings("beta", "resp_1", &["call_1".to_string()]);
        let body = serde_json::to_vec(&serde_json::json!({
            "input": [{
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "done"
            }]
        }))
        .unwrap();

        let attempts = pool.select_attempts(&body, &[]).unwrap();

        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].profile_name, "beta");
        assert!(attempts[0].hard_affinity);
    }

    #[test]
    fn gemini_binding_recorder_reads_responses_body() {
        let captured = Arc::new(Mutex::new(None::<(String, Vec<String>)>));
        let captured_for_recorder = Arc::clone(&captured);
        let recorder: RuntimeGeminiBindingRecorder = Arc::new(move |response_id, call_ids| {
            *captured_for_recorder.lock().unwrap() = Some((response_id, call_ids));
        });
        let body = serde_json::to_vec(&serde_json::json!({
            "id": "resp_1",
            "output": [{
                "type": "function_call",
                "call_id": "call_1",
                "name": "shell",
                "arguments": "{}"
            }]
        }))
        .unwrap();

        runtime_gemini_remember_bindings_from_responses_body(Some(&recorder), &body);

        let (response_id, call_ids) = captured.lock().unwrap().clone().unwrap();
        assert_eq!(response_id, "resp_1");
        assert_eq!(call_ids, vec!["call_1"]);
    }

    #[test]
    fn gemini_google_resource_exhausted_is_quota_blocked() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": 429,
                "message": "Quota exceeded for quota metric.",
                "status": "RESOURCE_EXHAUSTED"
            }
        }))
        .unwrap();

        assert_eq!(
            runtime_gemini_google_quota_message(&body).as_deref(),
            Some("Quota exceeded for quota metric.")
        );
    }

    #[test]
    fn gemini_quota_rotation_predicate_respects_affinity_and_attempt_budget() {
        assert!(runtime_gemini_should_rotate_after_quota_response(
            429, false, 0, 2
        ));
        assert!(!runtime_gemini_should_rotate_after_quota_response(
            429, true, 0, 2
        ));
        assert!(!runtime_gemini_should_rotate_after_quota_response(
            429, false, 1, 2
        ));
        assert!(!runtime_gemini_should_rotate_after_quota_response(
            500, false, 0, 2
        ));
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
        assert!(output.contains("\"arguments\":\"{\\\"cmd\\\":\\\"ls\\\"}\""));
        assert!(output.contains("event: response.completed"));
    }
}
