use super::{RuntimeRotationProxyShared, RuntimeRouteKind};

pub(crate) fn log_runtime_upstream_payload_snapshot(
    _shared: &RuntimeRotationProxyShared,
    _request_id: u64,
    transport: &str,
    route_kind: RuntimeRouteKind,
    _profile_name: &str,
    _payload: &[u8],
) {
    // Legacy log readers may still parse existing entries; new request bodies stay out of logs.
    let _ = runtime_upstream_payload_logging_allowed(transport, route_kind);
}

fn runtime_upstream_payload_logging_allowed(
    _transport: &str,
    _route_kind: RuntimeRouteKind,
) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_request_payload_logging_is_disabled_by_default() {
        assert!(!runtime_upstream_payload_logging_allowed(
            "websocket",
            RuntimeRouteKind::Websocket
        ));
        assert!(!runtime_upstream_payload_logging_allowed(
            "websocket",
            RuntimeRouteKind::Responses
        ));
        assert!(!runtime_upstream_payload_logging_allowed(
            "http",
            RuntimeRouteKind::Websocket
        ));
        assert!(!runtime_upstream_payload_logging_allowed(
            "http",
            RuntimeRouteKind::Responses
        ));
        assert!(!runtime_upstream_payload_logging_allowed(
            "http",
            RuntimeRouteKind::Compact
        ));
        assert!(!runtime_upstream_payload_logging_allowed(
            "http",
            RuntimeRouteKind::Standard
        ));
    }
}
