#![cfg(feature = "mojo-rich")]

#[test]
fn real_mojo_native_event_error_classification_preserves_precedence() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    assert_eq!(
        prodex_mojo_core::rich::classify_native_event_error(
            None,
            false,
            Some("rate_limit_exceeded"),
            Some("insufficient_quota"),
            Some("invalid_api_key"),
        )
        .unwrap(),
        None
    );
    assert_eq!(
        prodex_mojo_core::rich::classify_native_event_error(
            Some("error"),
            true,
            Some("rate_limit_exceeded"),
            Some("insufficient_quota"),
            Some("invalid_api_key"),
        )
        .unwrap(),
        Some(2)
    );
    assert_eq!(
        prodex_mojo_core::rich::classify_native_event_error(
            None,
            true,
            None,
            Some("insufficient_quota"),
            None,
        )
        .unwrap(),
        Some(1)
    );
}
