#![cfg(all(feature = "mojo-runtime", prodex_mojo_required))]

use prodex_mojo_core::runtime_proxy_dispatch::{
    RuntimeProxyAdmissionAction, RuntimeProxyAdmissionPlan, RuntimeProxyErrorResponse,
    admission_plan, content_length_value, error_response,
};

#[test]
fn runtime_proxy_dispatch_header_boundary_matrix_is_mojo_owned() {
    for (value, expected) in [
        ("", None),
        ("   ", None),
        ("malformed", None),
        ("+0007", Some(7)),
        ("64", Some(64)),
        (" \u{2003}64\u{2003} ", Some(64)),
        ("18446744073709551615", Some(u64::MAX)),
        ("18446744073709551616", None),
    ] {
        assert_eq!(
            content_length_value(value).unwrap(),
            expected,
            "value={value:?}"
        );
    }
}

#[test]
fn runtime_proxy_dispatch_admission_matrix_preserves_saturation_precedence() {
    let cases = [
        (
            1,
            0,
            false,
            false,
            RuntimeProxyAdmissionPlan {
                action: RuntimeProxyAdmissionAction::Reject,
                marks_global_overload: true,
            },
        ),
        (
            2,
            0,
            false,
            false,
            RuntimeProxyAdmissionPlan {
                action: RuntimeProxyAdmissionAction::CaptureAndRetry,
                marks_global_overload: true,
            },
        ),
        (
            2,
            1,
            false,
            false,
            RuntimeProxyAdmissionPlan {
                action: RuntimeProxyAdmissionAction::CaptureAndRetry,
                marks_global_overload: false,
            },
        ),
        (
            2,
            2,
            true,
            false,
            RuntimeProxyAdmissionPlan {
                action: RuntimeProxyAdmissionAction::Reject,
                marks_global_overload: false,
            },
        ),
        (
            2,
            3,
            false,
            true,
            RuntimeProxyAdmissionPlan {
                action: RuntimeProxyAdmissionAction::Reject,
                marks_global_overload: false,
            },
        ),
    ];
    for (rejection, lane, websocket, captured, expected) in cases {
        assert_eq!(
            admission_plan(rejection, lane, websocket, captured).unwrap(),
            expected,
            "rejection={rejection} lane={lane} websocket={websocket} captured={captured}"
        );
    }
}

#[test]
fn runtime_proxy_dispatch_error_matrix_preserves_local_response_classes() {
    assert_eq!(
        error_response(0).unwrap(),
        RuntimeProxyErrorResponse::BodyTooLarge
    );
    assert_eq!(
        error_response(1).unwrap(),
        RuntimeProxyErrorResponse::CaptureFailed
    );
    assert_eq!(
        error_response(2).unwrap(),
        RuntimeProxyErrorResponse::TransportFailed
    );
    assert_eq!(
        error_response(3).unwrap(),
        RuntimeProxyErrorResponse::RewriteFailed
    );
}
