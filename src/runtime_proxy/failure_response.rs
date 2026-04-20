use super::*;

pub(crate) fn runtime_proxy_precommit_budget_exhausted(
    started_at: Instant,
    attempts: usize,
    continuation: bool,
    pressure_mode: bool,
) -> bool {
    let (attempt_limit, budget) = runtime_proxy_precommit_budget(continuation, pressure_mode);

    attempts >= attempt_limit || started_at.elapsed() >= budget
}

pub(crate) fn runtime_proxy_final_retryable_http_failure_response(
    last_failure: Option<(tiny_http::ResponseBox, bool)>,
    saw_inflight_saturation: bool,
    json_errors: bool,
) -> Option<tiny_http::ResponseBox> {
    let service_unavailable = |message: &str| {
        if json_errors {
            build_runtime_proxy_json_error_response(503, "service_unavailable", message)
        } else {
            build_runtime_proxy_text_response(503, message)
        }
    };
    match last_failure {
        Some((response, false)) => Some(response),
        Some((_response, true)) if saw_inflight_saturation => Some(service_unavailable(
            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
        )),
        Some((_response, true)) => Some(service_unavailable(
            runtime_proxy_local_selection_failure_message(),
        )),
        None if saw_inflight_saturation => Some(service_unavailable(
            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
        )),
        None => None,
    }
}

pub(crate) fn runtime_proxy_final_responses_failure_reply(
    last_failure: Option<(RuntimeUpstreamFailureResponse, bool)>,
    saw_inflight_saturation: bool,
) -> RuntimeResponsesReply {
    match last_failure {
        Some((failure, false)) => match failure {
            RuntimeUpstreamFailureResponse::Http(response) => response,
            RuntimeUpstreamFailureResponse::Websocket(_) => {
                RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                ))
            }
        },
        _ if saw_inflight_saturation => {
            RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                503,
                "service_unavailable",
                "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
            ))
        }
        _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
            503,
            "service_unavailable",
            runtime_proxy_local_selection_failure_message(),
        )),
    }
}

pub(crate) fn send_runtime_proxy_final_websocket_failure(
    local_socket: &mut RuntimeLocalWebSocket,
    last_failure: Option<(RuntimeUpstreamFailureResponse, bool)>,
    saw_inflight_saturation: bool,
) -> Result<()> {
    match last_failure {
        Some((failure, false)) => match failure {
            RuntimeUpstreamFailureResponse::Websocket(payload)
                if runtime_websocket_error_payload_is_previous_response_not_found(&payload) =>
            {
                send_runtime_proxy_stale_continuation_websocket_error(local_socket)
            }
            RuntimeUpstreamFailureResponse::Websocket(payload) => {
                forward_runtime_proxy_websocket_error(local_socket, &payload)
            }
            RuntimeUpstreamFailureResponse::Http(_) => send_runtime_proxy_websocket_error(
                local_socket,
                503,
                "service_unavailable",
                runtime_proxy_local_selection_failure_message(),
            ),
        },
        _ if saw_inflight_saturation => send_runtime_proxy_websocket_error(
            local_socket,
            503,
            "service_unavailable",
            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
        ),
        _ => send_runtime_proxy_websocket_error(
            local_socket,
            503,
            "service_unavailable",
            runtime_proxy_local_selection_failure_message(),
        ),
    }
}

pub(crate) fn runtime_websocket_error_payload_is_previous_response_not_found(
    payload: &RuntimeWebsocketErrorPayload,
) -> bool {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => {
            extract_runtime_proxy_previous_response_message(text.as_bytes()).is_some()
        }
        RuntimeWebsocketErrorPayload::Binary(bytes) => {
            extract_runtime_proxy_previous_response_message(bytes).is_some()
        }
        RuntimeWebsocketErrorPayload::Empty => false,
    }
}

pub(crate) fn send_runtime_proxy_stale_continuation_websocket_error(
    local_socket: &mut RuntimeLocalWebSocket,
) -> Result<()> {
    send_runtime_proxy_websocket_error(
        local_socket,
        409,
        "stale_continuation",
        "Upstream no longer recognizes this conversation chain before output started. Retry from the last user message or restart the Codex turn; Prodex will not send a fresh request without the missing context.",
    )
}
