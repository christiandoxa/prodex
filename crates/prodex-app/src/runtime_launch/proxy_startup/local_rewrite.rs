use super::deepseek_rewrite::*;
use super::*;

pub(crate) const RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH: &str = "/v1";
const RUNTIME_LOCAL_REWRITE_PROFILE: &str = "local";

#[derive(Clone)]
struct RuntimeLocalRewriteProxyShared {
    runtime_shared: RuntimeRotationProxyShared,
    upstream_base_url: String,
    mount_path: String,
    provider: RuntimeLocalRewriteProviderOptions,
    deepseek_conversations: RuntimeDeepSeekConversationStore,
    deepseek_pending_messages: RuntimeDeepSeekPendingMessages,
    client: reqwest::blocking::Client,
}

#[derive(Clone)]
pub(crate) enum RuntimeLocalRewriteProviderOptions {
    OpenAiResponses,
    DeepSeek { api_key: String },
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
    let shared = RuntimeLocalRewriteProxyShared {
        runtime_shared: runtime_shared.clone(),
        upstream_base_url,
        mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        provider,
        deepseek_conversations: Arc::new(Mutex::new(BTreeMap::new())),
        deepseek_pending_messages: Arc::new(Mutex::new(BTreeMap::new())),
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
    ) && path_without_query(&captured.path_and_query).ends_with("/responses/compact")
    {
        let _ = request.respond(build_runtime_proxy_text_response(
            501,
            "DeepSeek provider does not support Codex remote compact yet",
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

fn send_runtime_local_rewrite_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<reqwest::blocking::Response> {
    let route_kind = runtime_local_rewrite_route_kind(&request.path_and_query);
    let body = prepare_runtime_smart_context_http_body(
        request_id,
        request,
        &shared.runtime_shared,
        route_kind,
    )
    .into_owned();
    let body = match &shared.provider {
        RuntimeLocalRewriteProviderOptions::OpenAiResponses => body,
        RuntimeLocalRewriteProviderOptions::DeepSeek { .. } => {
            if path_without_query(&request.path_and_query).ends_with("/responses") {
                let translated =
                    runtime_deepseek_chat_request_body(&body, &shared.deepseek_conversations)?;
                if let Ok(mut pending) = shared.deepseek_pending_messages.lock() {
                    pending.insert(request_id, translated.messages);
                }
                translated.body
            } else {
                body
            }
        }
    };
    let upstream_url = match &shared.provider {
        RuntimeLocalRewriteProviderOptions::OpenAiResponses => runtime_local_rewrite_upstream_url(
            &shared.upstream_base_url,
            &shared.mount_path,
            &request.path_and_query,
        ),
        RuntimeLocalRewriteProviderOptions::DeepSeek { .. } => runtime_deepseek_upstream_url(
            &shared.upstream_base_url,
            &shared.mount_path,
            &request.path_and_query,
        ),
    };
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime local rewrite",
            request.method
        )
    })?;
    let mut upstream_request = shared.client.request(method, &upstream_url);
    match &shared.provider {
        RuntimeLocalRewriteProviderOptions::OpenAiResponses => {
            for (name, value) in &request.headers {
                if should_skip_runtime_local_rewrite_request_header(name) {
                    continue;
                }
                upstream_request = upstream_request.header(name.as_str(), value.as_str());
            }
        }
        RuntimeLocalRewriteProviderOptions::DeepSeek { api_key } => {
            upstream_request = upstream_request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .bearer_auth(api_key);
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
                runtime_proxy_log_field("url", upstream_url.as_str()),
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

fn respond_runtime_local_rewrite_proxy_request(
    request_id: u64,
    request: tiny_http::Request,
    response: reqwest::blocking::Response,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) {
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
        assert_eq!(translated["messages"][2]["role"], "tool");
        assert_eq!(translated["messages"][2]["tool_call_id"], "call_1");
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
