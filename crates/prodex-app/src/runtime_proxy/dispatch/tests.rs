use super::{
    RUNTIME_PROXY_REQUEST_CAPTURE_FAILED_MESSAGE, RUNTIME_PROXY_REQUEST_REWRITE_FAILED_MESSAGE,
    RuntimeProxyAdmissionRejection, RuntimeRouteKind, runtime_proxy_capture_error_response,
    runtime_proxy_content_length_value, runtime_proxy_dispatch_admission_plan,
    runtime_proxy_dispatch_error_log_value, runtime_proxy_max_request_body_bytes,
    runtime_proxy_request_body_exceeds_limit, runtime_proxy_upstream_error_response,
};

#[test]
fn runtime_proxy_max_request_body_bytes_rejects_invalid_env_values() {
    for (value, message) in [
        (
            "",
            "PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES cannot be empty",
        ),
        (
            " 64 ",
            "PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES must not contain whitespace",
        ),
        (
            "not-a-number",
            "PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES must be an unsigned integer",
        ),
        (
            "0",
            "PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES must be greater than zero",
        ),
    ] {
        let _guard = crate::test_support::TestEnvVarGuard::set(
            "PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES",
            value,
        );

        let err = runtime_proxy_max_request_body_bytes().unwrap_err();

        assert!(err.to_string().contains(message));
    }
}

#[test]
fn runtime_proxy_body_limit_mojo_is_used_for_declared_and_observed_lengths() {
    for limit in [0, 1, 64, 65, (1_u64 << 63) - 1, 1_u64 << 63, u64::MAX] {
        for observed in [0, 1, 64, 65, 1_u64 << 63, u64::MAX] {
            assert_eq!(
                runtime_proxy_request_body_exceeds_limit(limit, observed).unwrap(),
                observed > limit,
                "limit={limit}, observed={observed}"
            );
        }
    }
}

#[test]
fn runtime_proxy_dispatch_consumer_preserves_empty_malformed_and_exact_headers() {
    for value in ["", "  ", "malformed", "18446744073709551616"] {
        assert_eq!(runtime_proxy_content_length_value(value).unwrap(), None);
    }
    assert_eq!(
        runtime_proxy_content_length_value(" +64 ").unwrap(),
        Some(64)
    );
    assert_eq!(
        runtime_proxy_content_length_value(" \u{2003}64\u{2003} ").unwrap(),
        Some(64)
    );
    assert_eq!(
        runtime_proxy_content_length_value("18446744073709551615").unwrap(),
        Some(u64::MAX)
    );
}

#[test]
fn runtime_proxy_dispatch_consumer_preserves_admission_and_error_precedence() {
    let responses_lane = runtime_proxy_dispatch_admission_plan(
        RuntimeProxyAdmissionRejection::LaneLimit(RuntimeRouteKind::Responses),
        false,
        false,
    );
    assert_eq!(
        responses_lane.action,
        prodex_mojo_core::runtime_proxy_dispatch::RuntimeProxyAdmissionAction::CaptureAndRetry
    );
    assert!(responses_lane.marks_global_overload);

    let websocket_lane = runtime_proxy_dispatch_admission_plan(
        RuntimeProxyAdmissionRejection::LaneLimit(RuntimeRouteKind::Websocket),
        true,
        false,
    );
    assert_eq!(
        websocket_lane.action,
        prodex_mojo_core::runtime_proxy_dispatch::RuntimeProxyAdmissionAction::Reject
    );
    assert!(!websocket_lane.marks_global_overload);

    assert_eq!(
        runtime_proxy_capture_error_response(true),
        prodex_mojo_core::runtime_proxy_dispatch::RuntimeProxyErrorResponse::BodyTooLarge
    );
    assert_eq!(
        runtime_proxy_upstream_error_response(true),
        prodex_mojo_core::runtime_proxy_dispatch::RuntimeProxyErrorResponse::TransportFailed
    );
    assert_eq!(
        runtime_proxy_upstream_error_response(false),
        prodex_mojo_core::runtime_proxy_dispatch::RuntimeProxyErrorResponse::RewriteFailed
    );
}

#[test]
fn runtime_proxy_max_request_body_bytes_accepts_exact_env_value() {
    let _guard = crate::test_support::TestEnvVarGuard::set(
        "PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES",
        "64",
    );

    assert_eq!(runtime_proxy_max_request_body_bytes().unwrap(), 64);
}

#[test]
fn dispatch_local_error_messages_are_stable_and_redacted() {
    for message in [
        RUNTIME_PROXY_REQUEST_CAPTURE_FAILED_MESSAGE,
        RUNTIME_PROXY_REQUEST_REWRITE_FAILED_MESSAGE,
    ] {
        assert!(!message.contains("token"));
        assert!(!message.contains("Authorization"));
        assert!(!message.contains("127.0.0.1"));
        assert!(!message.contains("serde"));
        assert!(!message.contains("json"));
        assert!(!message.contains("reqwest"));
    }
    assert_eq!(
        RUNTIME_PROXY_REQUEST_CAPTURE_FAILED_MESSAGE,
        "proxied request could not be captured"
    );
    assert_eq!(
        RUNTIME_PROXY_REQUEST_REWRITE_FAILED_MESSAGE,
        "proxied request could not be prepared"
    );
}

#[test]
fn dispatch_error_log_value_redacts_secret_like_material() {
    let message = runtime_proxy_dispatch_error_log_value(
        "proxy failed: Authorization: Bearer dispatch-token and api_key=dispatch-key",
    );

    assert!(message.contains("Authorization: Bearer <redacted>"));
    assert!(message.contains("api_key=<redacted>"));
    assert!(!message.contains("dispatch-token"));
    assert!(!message.contains("dispatch-key"));
}
