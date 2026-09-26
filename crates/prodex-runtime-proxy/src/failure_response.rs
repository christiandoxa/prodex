use std::time::{Duration, Instant};

use crate::{
    RuntimeBufferedResponseParts, RuntimeWebsocketErrorPayload,
    build_runtime_proxy_json_error_parts, extract_runtime_proxy_previous_response_message,
};

pub const RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT: usize = if cfg!(test) { 4 } else { 12 };
// Each profile may consume one bounded same-profile retry before rotation.
pub const RUNTIME_PROXY_PRECOMMIT_ATTEMPTS_PER_PROFILE: usize = 2;
pub const RUNTIME_PROXY_PRECOMMIT_BUDGET_MS: u64 = if cfg!(test) { 1_000 } else { 3_000 };
pub const RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT: usize =
    RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT * 2;
pub const RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS: u64 =
    RUNTIME_PROXY_PRECOMMIT_BUDGET_MS * 4;
pub const RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS: u64 = if cfg!(test) { 150 } else { 800 };
pub const RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT: usize = if cfg!(test) { 3 } else { 6 };
/// Additional bounded time available after a sweep finds only transiently unavailable profiles.
pub const RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS: u64 = if cfg!(test) { 5_000 } else { 30_000 };
pub const RUNTIME_PROXY_PRECOMMIT_RECOVERY_SWEEP_LIMIT: usize = 2;

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
    if parts.status != 429 && extract_runtime_proxy_previous_response_message(&parts.body).is_some()
    {
        runtime_proxy_stale_continuation_http_parts()
    } else {
        parts
    }
}

pub fn runtime_proxy_precommit_budget(
    continuation: bool,
    pressure_mode: bool,
) -> (usize, Duration) {
    runtime_proxy_precommit_budget_for_profile_count(continuation, pressure_mode, 1)
}

pub fn runtime_proxy_precommit_budget_for_profile_count(
    continuation: bool,
    pressure_mode: bool,
    profile_count: usize,
) -> (usize, Duration) {
    let plan = prodex_mojo_core::runtime::precommit_budget_plan(
        continuation,
        pressure_mode,
        RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
        RUNTIME_PROXY_PRECOMMIT_BUDGET_MS,
        RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT,
        RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS,
        RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT,
        RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS,
        profile_count,
        RUNTIME_PROXY_PRECOMMIT_ATTEMPTS_PER_PROFILE,
    )
    .expect("Mojo precommit budget planning returned an invalid result");
    (plan.attempt_limit, Duration::from_millis(plan.budget_ms))
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

pub fn runtime_proxy_precommit_budget_exhausted_for_profile_count(
    started_at: Instant,
    attempts: usize,
    continuation: bool,
    pressure_mode: bool,
    profile_count: usize,
) -> bool {
    let (attempt_limit, budget) = runtime_proxy_precommit_budget_for_profile_count(
        continuation,
        pressure_mode,
        profile_count,
    );
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
        runtime_proxy_local_capacity_timeout_message()
    } else {
        local_selection_failure_message
    }
}

pub fn runtime_proxy_local_capacity_timeout_message() -> &'static str {
    "Runtime proxy local capacity remained saturated until the request deadline. Retry the request."
}

#[cfg(test)]
#[path = "../tests/src/failure_response.rs"]
mod tests;
