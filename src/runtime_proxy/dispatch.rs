use super::*;

fn runtime_proxy_log_dispatch_error(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    event: &str,
    error: String,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            event,
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("error", error),
            ],
        ),
    );
}

pub(crate) fn handle_runtime_rotation_proxy_request(
    mut request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) {
    if let Some(response) = handle_runtime_proxy_admin_request(&mut request, shared) {
        let _ = request.respond(response);
        return;
    }
    if let Some(response) = handle_runtime_proxy_anthropic_compat_request(&request) {
        let _ = request.respond(response);
        return;
    }

    let request_path = request.url().to_string();
    let websocket = is_tiny_http_websocket_upgrade(&request);
    let request_transport = if websocket { "websocket" } else { "http" };
    let _active_request_guard = match acquire_runtime_proxy_active_request_slot_with_wait(
        shared,
        request_transport,
        &request_path,
    ) {
        Ok(guard) => guard,
        Err(RuntimeProxyAdmissionRejection::GlobalLimit) => {
            mark_runtime_proxy_local_overload(shared, "active_request_limit");
            reject_runtime_proxy_overloaded_request(request, shared, "active_request_limit");
            return;
        }
        Err(RuntimeProxyAdmissionRejection::LaneLimit(lane)) => {
            let reason = format!("lane_limit:{}", runtime_route_kind_label(lane));
            if runtime_proxy_lane_limit_marks_global_overload(lane) {
                mark_runtime_proxy_local_overload(shared, &reason);
            }
            reject_runtime_proxy_overloaded_request(request, shared, &reason);
            return;
        }
    };

    let request_id = runtime_proxy_next_request_id(shared);
    if websocket {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket upgrade path={}",
                request.url()
            ),
        );
        proxy_runtime_responses_websocket_request(request_id, request, shared);
        return;
    }

    dispatch_runtime_http_proxy_request(request_id, request, shared);
}

fn dispatch_runtime_http_proxy_request(
    request_id: u64,
    mut request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) {
    let captured = match capture_runtime_proxy_request(&mut request) {
        Ok(captured) => captured,
        Err(err) => {
            runtime_proxy_log_dispatch_error(shared, request_id, "capture_error", err.to_string());
            let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
            return;
        }
    };

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http path={} previous_response_id={:?} turn_state={:?} body_bytes={}",
            captured.path_and_query,
            runtime_request_previous_response_id(&captured),
            runtime_request_turn_state(&captured),
            captured.body.len()
        ),
    );
    let compat_surface = runtime_detect_request_compatibility_surface(&captured, "request", "http");
    runtime_proxy_log_request_compatibility(shared, request_id, &compat_surface);
    if is_runtime_anthropic_messages_path(&captured.path_and_query)
        && std::env::var_os("PRODEX_DEBUG_ANTHROPIC_COMPAT").is_some()
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http anthropic_compat headers={:?} body_snippet={}",
                captured.headers,
                runtime_proxy_body_snippet(&captured.body, 1024),
            ),
        );
    }

    if is_runtime_anthropic_messages_path(&captured.path_and_query) {
        let response = match proxy_runtime_anthropic_messages_request(request_id, &captured, shared)
        {
            Ok(response) => response,
            Err(err) => {
                if is_runtime_proxy_transport_failure(&err) {
                    runtime_proxy_log_dispatch_error(
                        shared,
                        request_id,
                        "anthropic_transport_failure",
                        format!("{err:#}"),
                    );
                    return;
                } else {
                    runtime_proxy_log_dispatch_error(
                        shared,
                        request_id,
                        "anthropic_error",
                        format!("{err:#}"),
                    );
                    RuntimeResponsesReply::Buffered(build_runtime_anthropic_error_parts(
                        502,
                        "api_error",
                        &err.to_string(),
                    ))
                }
            }
        };
        respond_runtime_responses_reply(request, response);
        return;
    }

    if is_runtime_responses_path(&captured.path_and_query) {
        let response = match proxy_runtime_responses_request(request_id, &captured, shared) {
            Ok(response) => response,
            Err(err) => {
                if is_runtime_proxy_transport_failure(&err) {
                    runtime_proxy_log_dispatch_error(
                        shared,
                        request_id,
                        "responses_transport_failure",
                        format!("{err:#}"),
                    );
                    return;
                } else {
                    runtime_proxy_log_dispatch_error(
                        shared,
                        request_id,
                        "responses_error",
                        format!("{err:#}"),
                    );
                    RuntimeResponsesReply::Buffered(build_runtime_proxy_text_response_parts(
                        502,
                        &err.to_string(),
                    ))
                }
            }
        };
        respond_runtime_responses_reply(request, response);
        return;
    }

    let response = match proxy_runtime_standard_request(request_id, &captured, shared) {
        Ok(response) => response,
        Err(err) => {
            if is_runtime_proxy_transport_failure(&err) {
                runtime_proxy_log_dispatch_error(
                    shared,
                    request_id,
                    "standard_transport_failure",
                    format!("{err:#}"),
                );
                return;
            } else {
                runtime_proxy_log_dispatch_error(
                    shared,
                    request_id,
                    "standard_error",
                    format!("{err:#}"),
                );
                build_runtime_proxy_text_response(502, &err.to_string())
            }
        }
    };
    let _ = request.respond(response);
}

pub(crate) fn respond_runtime_responses_reply(
    request: tiny_http::Request,
    response: RuntimeResponsesReply,
) {
    match response {
        RuntimeResponsesReply::Buffered(parts) => {
            let _ = request.respond(build_runtime_proxy_response_from_parts(parts));
        }
        RuntimeResponsesReply::Streaming(response) => {
            let writer = request.into_writer();
            let _ = write_runtime_streaming_response(writer, response);
        }
    }
}

pub(crate) fn is_tiny_http_websocket_upgrade(request: &tiny_http::Request) -> bool {
    request.headers().iter().any(|header| {
        header.field.equiv("Upgrade") && header.value.as_str().eq_ignore_ascii_case("websocket")
    })
}

pub(crate) fn capture_runtime_proxy_request(
    request: &mut tiny_http::Request,
) -> Result<RuntimeProxyRequest> {
    let mut body = Vec::new();
    request
        .as_reader()
        .read_to_end(&mut body)
        .context("failed to read proxied Codex request body")?;

    Ok(RuntimeProxyRequest {
        method: request.method().as_str().to_string(),
        path_and_query: request.url().to_string(),
        headers: runtime_proxy_request_headers(request),
        body,
    })
}

pub(crate) fn capture_runtime_proxy_websocket_request(
    request: &tiny_http::Request,
) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: request.method().as_str().to_string(),
        path_and_query: request.url().to_string(),
        headers: runtime_proxy_request_headers(request),
        body: Vec::new(),
    }
}

pub(crate) fn runtime_proxy_request_headers(request: &tiny_http::Request) -> Vec<(String, String)> {
    request
        .headers()
        .iter()
        .map(|header| {
            (
                header.field.as_str().as_str().to_string(),
                header.value.as_str().to_string(),
            )
        })
        .collect()
}

pub(crate) fn proxy_runtime_responses_websocket_request(
    request_id: u64,
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) {
    if !is_runtime_responses_path(request.url())
        && !is_runtime_realtime_websocket_path(request.url())
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket unsupported_path={}",
                request.url()
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            404,
            "Runtime websocket proxy only supports Codex responses and realtime endpoints.",
        ));
        return;
    }

    let handshake_request = capture_runtime_proxy_websocket_request(&request);
    let Some(websocket_key) = runtime_proxy_websocket_key(&handshake_request) else {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket missing_sec_websocket_key path={}",
                handshake_request.path_and_query
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            400,
            "Missing Sec-WebSocket-Key header for runtime auto-rotate websocket proxy.",
        ));
        return;
    };

    let response = build_runtime_proxy_websocket_upgrade_response(&websocket_key);
    let upgraded = request.upgrade("websocket", response);
    let mut local_socket = WsSocket::from_raw_socket(upgraded, WsRole::Server, None);
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=websocket upgraded path={} previous_response_id={:?} turn_state={:?}",
            handshake_request.path_and_query,
            runtime_request_previous_response_id(&handshake_request),
            runtime_request_turn_state(&handshake_request)
        ),
    );
    let compat_surface =
        runtime_detect_request_compatibility_surface(&handshake_request, "handshake", "websocket");
    runtime_proxy_log_request_compatibility(shared, request_id, &compat_surface);
    if let Err(err) = run_runtime_proxy_websocket_session(
        request_id,
        &mut local_socket,
        &handshake_request,
        shared,
    ) {
        runtime_proxy_log(
            shared,
            format!("request={request_id} transport=websocket session_error={err:#}"),
        );
        if !is_runtime_proxy_transport_failure(&err) {
            let _ = local_socket.close(None);
        }
    }
}

fn runtime_proxy_websocket_key(request: &RuntimeProxyRequest) -> Option<String> {
    request.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("Sec-WebSocket-Key")
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn build_runtime_proxy_websocket_upgrade_response(key: &str) -> TinyResponse<std::io::Empty> {
    let accept = derive_accept_key(key.as_bytes());
    TinyResponse::new_empty(TinyStatusCode(101))
        .with_header(TinyHeader::from_bytes("Upgrade", "websocket").expect("upgrade header"))
        .with_header(TinyHeader::from_bytes("Connection", "Upgrade").expect("connection header"))
        .with_header(
            TinyHeader::from_bytes("Sec-WebSocket-Accept", accept.as_bytes())
                .expect("accept header"),
        )
}

pub(crate) fn proxy_runtime_anthropic_messages_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    let translated_request = match translate_runtime_anthropic_messages_request(request) {
        Ok(translated_request) => translated_request,
        Err(err) => {
            return Ok(RuntimeResponsesReply::Buffered(
                build_runtime_anthropic_error_parts(400, "invalid_request_error", &err.to_string()),
            ));
        }
    };
    if std::env::var_os("PRODEX_DEBUG_ANTHROPIC_COMPAT").is_some() {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http anthropic_translated path={} headers={:?} body_snippet={}",
                translated_request.translated_request.path_and_query,
                translated_request.translated_request.headers,
                runtime_proxy_body_snippet(&translated_request.translated_request.body, 2048),
            ),
        );
    }
    let response = proxy_runtime_responses_request(
        request_id,
        &translated_request.translated_request,
        shared,
    )?;
    let translate_started_at = Instant::now();
    let translated_response = translate_runtime_responses_reply_to_anthropic(
        response,
        &translated_request,
        request_id,
        shared,
    )?;
    match &translated_response {
        RuntimeResponsesReply::Buffered(parts) => runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http anthropic_translate_complete stream={} needs_buffered_translation={} status={} content_type={} body_bytes={} elapsed_ms={}",
                translated_request.stream,
                translated_request.server_tools.needs_buffered_translation(),
                parts.status,
                runtime_buffered_response_content_type(parts).unwrap_or("-"),
                parts.body.len(),
                translate_started_at.elapsed().as_millis(),
            ),
        ),
        RuntimeResponsesReply::Streaming(response) => runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http anthropic_translate_complete stream={} needs_buffered_translation={} status={} body_streaming=true elapsed_ms={}",
                translated_request.stream,
                translated_request.server_tools.needs_buffered_translation(),
                response.status,
                translate_started_at.elapsed().as_millis(),
            ),
        ),
    }
    Ok(translated_response)
}
