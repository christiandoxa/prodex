use super::*;

use prodex_runtime_capabilities::runtime_capability_log_safe_value;
pub(super) use prodex_runtime_capabilities::{
    RuntimeRequestCompatibilitySurface, runtime_detect_request_compatibility_surface,
    runtime_detect_websocket_message_compatibility_surface,
};

pub(super) fn runtime_proxy_log_request_compatibility(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    surface: &RuntimeRequestCompatibilitySurface,
) {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} compat_request_surface stage={} family={} client={} route={} transport={} stream={} tool_surface={} continuation={} approval={} origin={} user_agent={}",
            surface.stage,
            surface.family,
            surface.client,
            surface.route,
            surface.transport,
            surface.stream,
            surface.tool_surface,
            surface.continuation,
            surface.approval,
            surface.request_origin,
            runtime_capability_log_safe_value(&surface.user_agent),
        ),
    );
    for warning in &surface.warnings {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} compat_warning stage={} family={} client={} route={} transport={} warning={warning}",
                surface.stage, surface.family, surface.client, surface.route, surface.transport
            ),
        );
    }
}
