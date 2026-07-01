use super::{
    RuntimeRotationProxyShared, RuntimeRouteKind, runtime_proxy_log, runtime_proxy_log_field,
    runtime_proxy_structured_log_message, runtime_route_kind_label,
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

const RUNTIME_UPSTREAM_PAYLOAD_LOG_LIMIT_BYTES: usize = 128 * 1024;

pub(crate) fn log_runtime_upstream_payload_snapshot(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    transport: &str,
    route_kind: RuntimeRouteKind,
    profile_name: &str,
    payload: &[u8],
) {
    if !runtime_upstream_payload_logging_allowed(transport, route_kind) {
        return;
    }
    let logged_len = payload.len().min(RUNTIME_UPSTREAM_PAYLOAD_LOG_LIMIT_BYTES);
    let payload_b64 = BASE64_STANDARD.encode(&payload[..logged_len]);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "upstream_payload",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("bytes", payload.len().to_string()),
                runtime_proxy_log_field("logged_bytes", logged_len.to_string()),
                runtime_proxy_log_field(
                    "truncated",
                    if logged_len < payload.len() {
                        "true"
                    } else {
                        "false"
                    },
                ),
                runtime_proxy_log_field("payload_b64", payload_b64),
            ],
        ),
    );
}

fn runtime_upstream_payload_logging_allowed(transport: &str, route_kind: RuntimeRouteKind) -> bool {
    !transport.eq_ignore_ascii_case("websocket") && route_kind != RuntimeRouteKind::Standard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_payload_logging_skips_websocket_request_text() {
        assert!(!runtime_upstream_payload_logging_allowed(
            "websocket",
            RuntimeRouteKind::Websocket
        ));
        assert!(runtime_upstream_payload_logging_allowed(
            "http",
            RuntimeRouteKind::Responses
        ));
        assert!(!runtime_upstream_payload_logging_allowed(
            "http",
            RuntimeRouteKind::Standard
        ));
    }
}
