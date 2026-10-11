#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime_broker_admin::{
    RuntimeBrokerAdminOperation, RuntimeBrokerAdminValueKind, method_is_post, value_span,
};

#[test]
fn strict_admin_method_adapter_rejects_wrong_methods_for_both_routes() {
    for operation in [
        RuntimeBrokerAdminOperation::Activate,
        RuntimeBrokerAdminOperation::ReleaseSessionAffinity,
    ] {
        assert!(method_is_post(operation, "POST").unwrap());
        assert!(!method_is_post(operation, "GET").unwrap());
    }
}

#[test]
fn strict_admin_value_adapter_preserves_trimmed_spans_and_rejections() {
    let value = "\u{2003}work\u{2003}";
    let (start, end) = value_span(RuntimeBrokerAdminValueKind::ActivationProfile, value)
        .unwrap()
        .expect("trimmed profile should be accepted");
    assert_eq!(&value[start..end], "work");
    assert_eq!(
        value_span(RuntimeBrokerAdminValueKind::SessionId, "\n\t").unwrap(),
        None
    );
    assert_eq!(
        value_span(RuntimeBrokerAdminValueKind::SessionId, &"a".repeat(257)).unwrap(),
        None
    );
    assert_eq!(
        value_span(
            RuntimeBrokerAdminValueKind::ActivationProfile,
            &"a".repeat(257)
        )
        .unwrap(),
        Some((0, 257))
    );
}
