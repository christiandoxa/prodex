use super::*;

pub(crate) use runtime_proxy_crate::{
    RuntimeProfileInFlightReleaseSnapshot, RuntimeProxyQueueRejection,
};

fn runtime_route_kind_to_proxy(
    route_kind: RuntimeRouteKind,
) -> runtime_proxy_crate::RuntimeRouteKind {
    match route_kind {
        RuntimeRouteKind::Responses => runtime_proxy_crate::RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact => runtime_proxy_crate::RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket => runtime_proxy_crate::RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard => runtime_proxy_crate::RuntimeRouteKind::Standard,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeProxyAdmissionRejection {
    GlobalLimit,
    LaneLimit(RuntimeRouteKind),
}

pub(crate) fn reject_runtime_proxy_overloaded_request(
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
    reason: &str,
) {
    let path = request.url().to_string();
    let websocket = is_tiny_http_websocket_upgrade(&request);
    let response = runtime_proxy_overloaded_response(shared, &path, websocket, reason);
    let _ = request.respond(response);
}

pub(crate) fn runtime_proxy_overloaded_response(
    shared: &RuntimeRotationProxyShared,
    path: &str,
    websocket: bool,
    reason: &str,
) -> tiny_http::ResponseBox {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "runtime_proxy_queue_overloaded",
            [
                runtime_proxy_log_field("transport", if websocket { "websocket" } else { "http" }),
                runtime_proxy_log_field("path", runtime_proxy_log_url(path)),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
    let response = if websocket {
        build_runtime_proxy_text_response(
            503,
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        )
    } else if is_runtime_anthropic_messages_path(path) {
        build_runtime_proxy_response_from_parts(build_runtime_anthropic_error_parts(
            503,
            runtime_anthropic_error_type_for_status(503),
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        ))
    } else if is_runtime_responses_path(path) || is_runtime_compact_path(path) {
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
    runtime_proxy_response_with_retry_after(response)
}

fn runtime_proxy_response_with_retry_after(
    mut response: tiny_http::ResponseBox,
) -> tiny_http::ResponseBox {
    let retry_after = RUNTIME_PROXY_LOCAL_OVERLOAD_BACKOFF_SECONDS
        .max(1)
        .to_string();
    if let Ok(header) = TinyHeader::from_bytes("Retry-After", retry_after.as_bytes()) {
        response = response.with_header(header);
    }
    response
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

pub(crate) fn record_runtime_profile_inflight_acquire(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    count: usize,
    weight: usize,
    context: &'static str,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_inflight",
            [
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("count", count.to_string()),
                runtime_proxy_log_field("weight", weight.to_string()),
                runtime_proxy_log_field("context", context),
                runtime_proxy_log_field("event", "acquire"),
            ],
        ),
    );
}

pub(crate) fn release_runtime_profile_inflight_guard(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &'static str,
    weight: usize,
) -> Option<RuntimeProfileInFlightReleaseSnapshot> {
    let (remaining, count_before, underflow) = shared
        .lane_admission
        .release_profile_inflight(profile_name, weight);
    if underflow {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "profile_inflight_underflow",
                [
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("count_before", count_before.to_string()),
                    runtime_proxy_log_field("weight", weight.to_string()),
                    runtime_proxy_log_field("context", context),
                ],
            ),
        );
    }
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_inflight",
            [
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("count", remaining.to_string()),
                runtime_proxy_log_field("weight", weight.to_string()),
                runtime_proxy_log_field("context", context),
                runtime_proxy_log_field("event", "release"),
            ],
        ),
    );
    Some(RuntimeProfileInFlightReleaseSnapshot {
        remaining,
        underflow,
    })
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

pub(crate) fn runtime_proxy_pressure_mode_for_route(
    route_kind: RuntimeRouteKind,
    local_overload_pressure: bool,
    background_queue_pressure: bool,
) -> bool {
    runtime_proxy_crate::runtime_proxy_pressure_mode_for_route(
        runtime_route_kind_to_proxy(route_kind),
        local_overload_pressure,
        background_queue_pressure,
    )
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
    runtime_proxy_crate::runtime_proxy_sync_probe_pressure_mode_for_route(
        runtime_route_kind_to_proxy(route_kind),
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
    runtime_proxy_crate::runtime_proxy_lane_limit_marks_global_overload(
        runtime_route_kind_to_proxy(lane),
    )
}

pub(crate) fn runtime_proxy_should_shed_fresh_compact_request(
    pressure_mode: bool,
    session_profile: Option<&str>,
) -> bool {
    runtime_proxy_crate::runtime_proxy_should_shed_fresh_compact_request(
        pressure_mode,
        session_profile,
    )
}

#[cfg(test)]
#[path = "../../tests/src/runtime_proxy/admission.rs"]
mod tests;
