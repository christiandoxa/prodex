use std::time::{Duration, Instant};

use crate::{
    RuntimeBufferedResponseParts, RuntimeWebsocketErrorPayload,
    build_runtime_proxy_json_error_parts, extract_runtime_proxy_previous_response_message,
};

pub const RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT: usize = if cfg!(test) { 4 } else { 12 };
pub const RUNTIME_PROXY_PRECOMMIT_BUDGET_MS: u64 = if cfg!(test) { 500 } else { 3_000 };
pub const RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT: usize =
    RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT * 2;
pub const RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS: u64 =
    RUNTIME_PROXY_PRECOMMIT_BUDGET_MS * 4;
pub const RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS: u64 = if cfg!(test) { 150 } else { 800 };
pub const RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT: usize = if cfg!(test) { 2 } else { 6 };

pub fn runtime_proxy_stale_continuation_message() -> &'static str {
    "Upstream no longer recognizes this conversation chain before output started. Retry from the last user message or restart the Codex turn; Prodex will not send a fresh request without the missing context."
}

pub fn runtime_proxy_stale_continuation_http_parts() -> RuntimeBufferedResponseParts {
    build_runtime_proxy_json_error_parts(
        409,
        "stale_continuation",
        runtime_proxy_stale_continuation_message(),
    )
}

pub fn runtime_proxy_translate_previous_response_http_parts(
    parts: RuntimeBufferedResponseParts,
) -> RuntimeBufferedResponseParts {
    if extract_runtime_proxy_previous_response_message(&parts.body).is_some() {
        runtime_proxy_stale_continuation_http_parts()
    } else {
        parts
    }
}

pub fn runtime_proxy_precommit_budget(
    continuation: bool,
    pressure_mode: bool,
) -> (usize, Duration) {
    if continuation {
        (
            RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS),
        )
    } else if pressure_mode {
        (
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS),
        )
    } else {
        (
            RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_BUDGET_MS),
        )
    }
}

pub fn runtime_proxy_precommit_budget_exhausted(
    started_at: Instant,
    attempts: usize,
    continuation: bool,
    pressure_mode: bool,
) -> bool {
    let (attempt_limit, budget) = runtime_proxy_precommit_budget(continuation, pressure_mode);

    attempts >= attempt_limit || started_at.elapsed() >= budget
}

pub fn runtime_websocket_error_payload_is_previous_response_not_found(
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

pub fn runtime_proxy_final_retryable_failure_message(
    saw_inflight_saturation: bool,
    local_selection_failure_message: &'static str,
) -> &'static str {
    if saw_inflight_saturation {
        "All runtime auto-rotate candidates are temporarily saturated. Retry the request."
    } else {
        local_selection_failure_message
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/crates/prodex-runtime-proxy/src/failure_response.rs"]
mod tests;
