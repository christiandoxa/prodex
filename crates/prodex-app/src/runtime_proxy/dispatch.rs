use super::*;

mod websocket;

pub(crate) use websocket::{
    capture_runtime_proxy_websocket_request, is_tiny_http_websocket_upgrade,
    proxy_runtime_responses_websocket_request,
};

const RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
struct RuntimeProxyRequestBodyTooLarge {
    limit: usize,
}

impl std::fmt::Display for RuntimeProxyRequestBodyTooLarge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "proxied Codex request body exceeds {} bytes",
            self.limit
        )
    }
}

impl std::error::Error for RuntimeProxyRequestBodyTooLarge {}

pub(crate) fn runtime_proxy_capture_error_status(err: &anyhow::Error) -> u16 {
    if err
        .downcast_ref::<RuntimeProxyRequestBodyTooLarge>()
        .is_some()
    {
        413
    } else {
        502
    }
}

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
    let request_id = runtime_proxy_next_request_id(shared);
    if !websocket
        && let Some(response) =
            runtime_startup_metadata_admission_pressure_response(request_id, &request_path, shared)
    {
        let _ = request.respond(response);
        return;
    }
    let mut captured_after_lane_limit = None;
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
        Err(RuntimeProxyAdmissionRejection::LaneLimit(_lane)) if !websocket => {
            let captured = match capture_runtime_proxy_request(&mut request) {
                Ok(captured) => captured,
                Err(err) => {
                    runtime_proxy_log_dispatch_error(
                        shared,
                        request_id,
                        "capture_error",
                        err.to_string(),
                    );
                    let _ = request.respond(build_runtime_proxy_text_response(
                        runtime_proxy_capture_error_status(&err),
                        &err.to_string(),
                    ));
                    return;
                }
            };
            match acquire_runtime_proxy_active_request_slot_with_wait_for_request(
                shared,
                request_transport,
                &request_path,
                Some(&captured),
            ) {
                Ok(guard) => {
                    captured_after_lane_limit = Some(captured);
                    guard
                }
                Err(RuntimeProxyAdmissionRejection::GlobalLimit) => {
                    mark_runtime_proxy_local_overload(shared, "active_request_limit");
                    reject_runtime_proxy_overloaded_request(
                        request,
                        shared,
                        "active_request_limit",
                    );
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
            }
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

    if websocket {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "upgrade",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("path", request.url()),
                ],
            ),
        );
        proxy_runtime_responses_websocket_request(request_id, request, shared);
        return;
    }

    dispatch_runtime_http_proxy_request(request_id, request, shared, captured_after_lane_limit);
}

fn dispatch_runtime_http_proxy_request(
    request_id: u64,
    mut request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
    captured: Option<RuntimeProxyRequest>,
) {
    let mut captured = match captured {
        Some(captured) => captured,
        None => match capture_runtime_proxy_request(&mut request) {
            Ok(captured) => captured,
            Err(err) => {
                runtime_proxy_log_dispatch_error(
                    shared,
                    request_id,
                    "capture_error",
                    err.to_string(),
                );
                let _ = request.respond(build_runtime_proxy_text_response(
                    runtime_proxy_capture_error_status(&err),
                    &err.to_string(),
                ));
                return;
            }
        },
    };
    if let Err(err) = apply_runtime_presidio_redaction_to_request(request_id, &mut captured, shared)
    {
        runtime_proxy_log_dispatch_error(
            shared,
            request_id,
            "presidio_redaction_failed",
            format!("{err:#}"),
        );
        let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
        return;
    }

    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "request_captured",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                runtime_proxy_log_field(
                    "previous_response_id",
                    format!("{:?}", runtime_request_previous_response_id(&captured)),
                ),
                runtime_proxy_log_field(
                    "turn_state",
                    format!("{:?}", runtime_request_turn_state(&captured)),
                ),
                runtime_proxy_log_field("body_bytes", captured.body.len().to_string()),
            ],
        ),
    );
    let compat_surface = runtime_detect_request_compatibility_surface(&captured, "request", "http");
    runtime_proxy_log_request_compatibility(shared, request_id, &compat_surface);
    if is_runtime_anthropic_messages_path(&captured.path_and_query)
        && std::env::var_os("PRODEX_DEBUG_ANTHROPIC_COMPAT").is_some()
    {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "anthropic_compat",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field(
                        "headers",
                        runtime_proxy_redacted_headers_debug(&captured.headers),
                    ),
                    runtime_proxy_log_field(
                        "body_snippet",
                        runtime_proxy_redacted_body_snippet(&captured.body, 1024),
                    ),
                ],
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

pub(crate) fn capture_runtime_proxy_request(
    request: &mut tiny_http::Request,
) -> Result<RuntimeProxyRequest> {
    if let Some(body_length) = request.body_length()
        && body_length > RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES
    {
        return Err(RuntimeProxyRequestBodyTooLarge {
            limit: RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES,
        }
        .into());
    }
    let body = read_runtime_proxy_request_body_limited(
        request.as_reader(),
        RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES,
    )?;

    Ok(RuntimeProxyRequest {
        method: request.method().as_str().to_string(),
        path_and_query: request.url().to_string(),
        headers: runtime_proxy_request_headers(request),
        body,
    })
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

fn read_runtime_proxy_request_body_limited(
    reader: impl std::io::Read,
    limit: usize,
) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut reader = reader.take((limit as u64).saturating_add(1));
    reader
        .read_to_end(&mut body)
        .context("failed to read proxied Codex request body")?;
    if body.len() > limit {
        return Err(RuntimeProxyRequestBodyTooLarge { limit }.into());
    }
    Ok(body)
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
            runtime_proxy_structured_log_message(
                "anthropic_translated",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field(
                        "path",
                        translated_request
                            .translated_request
                            .path_and_query
                            .as_str(),
                    ),
                    runtime_proxy_log_field(
                        "headers",
                        runtime_proxy_redacted_headers_debug(
                            &translated_request.translated_request.headers,
                        ),
                    ),
                    runtime_proxy_log_field(
                        "body_snippet",
                        runtime_proxy_redacted_body_snippet(
                            &translated_request.translated_request.body,
                            2048,
                        ),
                    ),
                ],
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
            runtime_proxy_structured_log_message(
                "anthropic_translate_complete",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("stream", translated_request.stream.to_string()),
                    runtime_proxy_log_field(
                        "needs_buffered_translation",
                        translated_request
                            .server_tools
                            .needs_buffered_translation()
                            .to_string(),
                    ),
                    runtime_proxy_log_field("status", parts.status.to_string()),
                    runtime_proxy_log_field(
                        "content_type",
                        runtime_buffered_response_content_type(parts).unwrap_or("-"),
                    ),
                    runtime_proxy_log_field("body_bytes", parts.body.len().to_string()),
                    runtime_proxy_log_field(
                        "elapsed_ms",
                        translate_started_at.elapsed().as_millis().to_string(),
                    ),
                ],
            ),
        ),
        RuntimeResponsesReply::Streaming(response) => runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "anthropic_translate_complete",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("stream", translated_request.stream.to_string()),
                    runtime_proxy_log_field(
                        "needs_buffered_translation",
                        translated_request
                            .server_tools
                            .needs_buffered_translation()
                            .to_string(),
                    ),
                    runtime_proxy_log_field("status", response.status.to_string()),
                    runtime_proxy_log_field("body_streaming", "true"),
                    runtime_proxy_log_field(
                        "elapsed_ms",
                        translate_started_at.elapsed().as_millis().to_string(),
                    ),
                ],
            ),
        ),
    }
    Ok(translated_response)
}
