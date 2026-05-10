use super::*;

#[cfg(test)]
pub(crate) use runtime_proxy_crate::{
    RuntimeHttpErrorAction, RuntimeHttpErrorClass, RuntimeHttpErrorPhase, runtime_http_error_policy,
};
pub(crate) use runtime_proxy_crate::{
    is_runtime_realtime_call_path, is_runtime_realtime_websocket_path,
    runtime_proxy_request_is_long_lived, runtime_proxy_request_prefers_inflight_wait,
    runtime_proxy_request_prefers_interactive_inflight_wait,
};

pub(crate) fn runtime_proxy_request_lane(path: &str, websocket: bool) -> RuntimeRouteKind {
    match runtime_proxy_crate::runtime_proxy_request_lane(path, websocket) {
        runtime_proxy_crate::RuntimeRouteKind::Responses => RuntimeRouteKind::Responses,
        runtime_proxy_crate::RuntimeRouteKind::Compact => RuntimeRouteKind::Compact,
        runtime_proxy_crate::RuntimeRouteKind::Websocket => RuntimeRouteKind::Websocket,
        runtime_proxy_crate::RuntimeRouteKind::Standard => RuntimeRouteKind::Standard,
    }
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
