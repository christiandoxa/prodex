use anyhow::Result;
use std::time::Instant;

use crate::{RuntimeLocalWebSocket, RuntimeUpstreamFailureResponse};

use super::{
    RuntimeHeapTrimmedBufferedResponseParts, RuntimeResponsesReply, RuntimeRotationProxyShared,
    build_runtime_proxy_json_error_parts, build_runtime_proxy_json_error_response,
    build_runtime_proxy_text_response, extract_runtime_proxy_previous_response_message,
    forward_runtime_proxy_websocket_error, runtime_proxy_local_selection_failure_message,
    send_runtime_proxy_websocket_error,
};

pub(crate) use runtime_proxy_crate::{
    runtime_proxy_stale_continuation_message,
    runtime_websocket_error_payload_is_previous_response_not_found,
};

#[cfg(test)]
pub(crate) use runtime_proxy_crate::runtime_proxy_precommit_budget_exhausted;

pub(crate) fn runtime_proxy_translate_previous_response_http_parts(
    parts: RuntimeHeapTrimmedBufferedResponseParts,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    if extract_runtime_proxy_previous_response_message(&parts.body).is_some() {
        RuntimeHeapTrimmedBufferedResponseParts::from_crate_parts(
            runtime_proxy_crate::runtime_proxy_stale_continuation_http_parts(),
        )
    } else {
        parts
    }
}

pub(crate) fn runtime_proxy_precommit_budget_exhausted_for_route(
    shared: &RuntimeRotationProxyShared,
    started_at: Instant,
    attempts: usize,
    continuation: bool,
    pressure_mode: bool,
) -> Result<bool> {
    let profile_count = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        runtime.state.profiles.len().max(1)
    };
    let (attempt_limit, budget) =
        runtime_proxy_crate::runtime_proxy_precommit_budget_for_profile_count(
            continuation,
            pressure_mode,
            profile_count,
        );

    Ok(attempts >= attempt_limit || started_at.elapsed() >= budget)
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
        Some((_, true)) => {
            let message = runtime_proxy_crate::runtime_proxy_final_retryable_failure_message(
                saw_inflight_saturation,
                runtime_proxy_local_selection_failure_message(),
            );
            Some(service_unavailable(message))
        }
        None if saw_inflight_saturation => {
            let message = runtime_proxy_crate::runtime_proxy_final_retryable_failure_message(
                saw_inflight_saturation,
                runtime_proxy_local_selection_failure_message(),
            );
            Some(service_unavailable(message))
        }
        None => None,
    }
}

pub(crate) fn runtime_proxy_final_responses_failure_reply(
    last_failure: Option<(RuntimeUpstreamFailureResponse, bool)>,
    saw_inflight_saturation: bool,
) -> RuntimeResponsesReply {
    match last_failure {
        Some((failure, false)) => match failure {
            RuntimeUpstreamFailureResponse::Http(response) => match response {
                RuntimeResponsesReply::Buffered(parts) => RuntimeResponsesReply::Buffered(
                    runtime_proxy_translate_previous_response_http_parts(parts),
                ),
                other => other,
            },
            RuntimeUpstreamFailureResponse::Websocket(_) => {
                RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                ))
            }
        },
        _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
            503,
            "service_unavailable",
            runtime_proxy_crate::runtime_proxy_final_retryable_failure_message(
                saw_inflight_saturation,
                runtime_proxy_local_selection_failure_message(),
            ),
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
        _ => send_runtime_proxy_websocket_error(
            local_socket,
            503,
            "service_unavailable",
            runtime_proxy_crate::runtime_proxy_final_retryable_failure_message(
                saw_inflight_saturation,
                runtime_proxy_local_selection_failure_message(),
            ),
        ),
    }
}

pub(crate) fn send_runtime_proxy_stale_continuation_websocket_error(
    local_socket: &mut RuntimeLocalWebSocket,
) -> Result<()> {
    send_runtime_proxy_websocket_error(
        local_socket,
        409,
        "stale_continuation",
        runtime_proxy_stale_continuation_message(),
    )
}
