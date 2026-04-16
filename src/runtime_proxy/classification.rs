use super::*;

pub(crate) fn runtime_proxy_request_lane(path: &str, websocket: bool) -> RuntimeRouteKind {
    if websocket {
        RuntimeRouteKind::Websocket
    } else if is_runtime_compact_path(path) {
        RuntimeRouteKind::Compact
    } else if is_runtime_responses_path(path) || is_runtime_anthropic_messages_path(path) {
        RuntimeRouteKind::Responses
    } else {
        RuntimeRouteKind::Standard
    }
}

pub(crate) fn runtime_proxy_request_origin(headers: &[(String, String)]) -> Option<&str> {
    runtime_proxy_request_header_value(headers, PRODEX_INTERNAL_REQUEST_ORIGIN_HEADER)
}

pub(crate) fn runtime_proxy_request_prefers_interactive_inflight_wait(
    request: &RuntimeProxyRequest,
) -> bool {
    runtime_proxy_request_origin(&request.headers).is_some_and(|origin| {
        origin.eq_ignore_ascii_case(PRODEX_INTERNAL_REQUEST_ORIGIN_ANTHROPIC_MESSAGES)
    })
}

pub(crate) fn runtime_proxy_request_prefers_inflight_wait(request: &RuntimeProxyRequest) -> bool {
    is_runtime_responses_path(&request.path_and_query)
        || runtime_proxy_request_prefers_interactive_inflight_wait(request)
}

pub(crate) fn is_runtime_realtime_call_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/realtime/calls")
}

pub(crate) fn is_runtime_realtime_websocket_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/realtime")
}

pub(crate) fn runtime_proxy_request_inflight_wait_budget(
    request: &RuntimeProxyRequest,
    pressure_mode: bool,
) -> Duration {
    if runtime_proxy_request_prefers_interactive_inflight_wait(request) {
        runtime_proxy_admission_wait_budget(RUNTIME_PROXY_ANTHROPIC_MESSAGES_PATH, pressure_mode)
    } else if runtime_proxy_request_prefers_inflight_wait(request) {
        runtime_proxy_admission_wait_budget(&request.path_and_query, pressure_mode)
    } else {
        Duration::ZERO
    }
}

pub(crate) fn runtime_proxy_request_is_long_lived(path: &str, websocket: bool) -> bool {
    websocket || is_runtime_responses_path(path) || is_runtime_anthropic_messages_path(path)
}

pub(crate) fn runtime_proxy_interactive_wait_budget_ms(path: &str, base_budget_ms: u64) -> u64 {
    if is_runtime_anthropic_messages_path(path) {
        base_budget_ms.saturating_mul(RUNTIME_PROXY_INTERACTIVE_WAIT_MULTIPLIER)
    } else {
        base_budget_ms
    }
}

pub(crate) fn runtime_proxy_admission_wait_budget(path: &str, pressure_mode: bool) -> Duration {
    let base_budget_ms = if pressure_mode {
        runtime_proxy_pressure_admission_wait_budget_ms()
    } else {
        runtime_proxy_admission_wait_budget_ms()
    };
    Duration::from_millis(runtime_proxy_interactive_wait_budget_ms(
        path,
        base_budget_ms,
    ))
}

pub(crate) fn runtime_proxy_long_lived_queue_wait_budget(
    path: &str,
    pressure_mode: bool,
) -> Duration {
    let base_budget_ms = if pressure_mode {
        runtime_proxy_pressure_long_lived_queue_wait_budget_ms()
    } else {
        runtime_proxy_long_lived_queue_wait_budget_ms()
    };
    Duration::from_millis(runtime_proxy_interactive_wait_budget_ms(
        path,
        base_budget_ms,
    ))
}
