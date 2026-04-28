use super::*;

pub(crate) fn reject_runtime_proxy_overloaded_request(
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
    reason: &str,
) {
    let path = request.url().to_string();
    let websocket = is_tiny_http_websocket_upgrade(&request);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "runtime_proxy_queue_overloaded",
            [
                runtime_proxy_log_field("transport", if websocket { "websocket" } else { "http" }),
                runtime_proxy_log_field("path", path.as_str()),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
    let response = if websocket {
        build_runtime_proxy_text_response(
            503,
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        )
    } else if is_runtime_anthropic_messages_path(&path) {
        build_runtime_proxy_response_from_parts(build_runtime_anthropic_error_parts(
            503,
            runtime_anthropic_error_type_for_status(503),
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        ))
    } else if is_runtime_responses_path(&path) || is_runtime_compact_path(&path) {
        build_runtime_proxy_json_error_response(
            503,
            "service_unavailable",
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        )
    } else {
        build_runtime_proxy_text_response(
            503,
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        )
    };
    let _ = request.respond(response);
}

pub(crate) fn mark_runtime_proxy_local_overload(shared: &RuntimeRotationProxyShared, reason: &str) {
    let now = Local::now().timestamp().max(0) as u64;
    let until = now.saturating_add(RUNTIME_PROXY_LOCAL_OVERLOAD_BACKOFF_SECONDS.max(1) as u64);
    let current = shared.local_overload_backoff_until.load(Ordering::SeqCst);
    if until > current {
        shared
            .local_overload_backoff_until
            .store(until, Ordering::SeqCst);
    }
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "runtime_proxy_overload_backoff",
            [
                runtime_proxy_log_field("until", until.to_string()),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeProxyAdmissionRejection {
    GlobalLimit,
    LaneLimit(RuntimeRouteKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeProxyQueueRejection {
    Full,
    Disconnected,
}

pub(crate) fn runtime_proxy_local_overload_pressure_active(
    shared: &RuntimeRotationProxyShared,
) -> bool {
    let now = Local::now().timestamp().max(0) as u64;
    shared.local_overload_backoff_until.load(Ordering::SeqCst) > now
}

pub(crate) fn runtime_proxy_background_queue_pressure_active() -> bool {
    runtime_proxy_queue_pressure_active(
        runtime_state_save_queue_backlog(),
        runtime_continuation_journal_queue_backlog(),
        runtime_probe_refresh_queue_backlog(),
    )
}

pub(crate) fn runtime_proxy_pressure_mode_active(shared: &RuntimeRotationProxyShared) -> bool {
    runtime_proxy_local_overload_pressure_active(shared)
        || runtime_proxy_background_queue_pressure_active()
}

pub(crate) fn runtime_proxy_background_queue_pressure_affects_route(
    route_kind: RuntimeRouteKind,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard
    )
}

pub(crate) fn runtime_proxy_pressure_mode_for_route(
    route_kind: RuntimeRouteKind,
    local_overload_pressure: bool,
    background_queue_pressure: bool,
) -> bool {
    local_overload_pressure
        || (background_queue_pressure
            && runtime_proxy_background_queue_pressure_affects_route(route_kind))
}

pub(crate) fn runtime_proxy_pressure_mode_active_for_route(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime_proxy_pressure_mode_for_route(
        route_kind,
        runtime_proxy_local_overload_pressure_active(shared),
        runtime_proxy_background_queue_pressure_active(),
    )
}

pub(crate) fn runtime_proxy_pressure_mode_active_for_request_path(
    shared: &RuntimeRotationProxyShared,
    path: &str,
    websocket: bool,
) -> bool {
    runtime_proxy_pressure_mode_active_for_route(
        shared,
        runtime_proxy_request_lane(path, websocket),
    )
}

pub(crate) fn runtime_proxy_sync_probe_pressure_mode_for_route(
    route_kind: RuntimeRouteKind,
    local_overload_pressure: bool,
    background_queue_pressure: bool,
) -> bool {
    runtime_proxy_pressure_mode_for_route(
        route_kind,
        local_overload_pressure,
        background_queue_pressure,
    )
}

pub(crate) fn runtime_proxy_sync_probe_pressure_mode_active_for_route(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime_proxy_sync_probe_pressure_mode_for_route(
        route_kind,
        runtime_proxy_local_overload_pressure_active(shared),
        runtime_proxy_background_queue_pressure_active(),
    )
}

pub(crate) fn runtime_proxy_lane_limit_marks_global_overload(lane: RuntimeRouteKind) -> bool {
    lane == RuntimeRouteKind::Responses
}

pub(crate) fn runtime_proxy_should_shed_fresh_compact_request(
    pressure_mode: bool,
    session_profile: Option<&str>,
) -> bool {
    pressure_mode && session_profile.is_none()
}
