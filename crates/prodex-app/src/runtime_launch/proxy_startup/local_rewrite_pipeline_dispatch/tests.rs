use super::{
    runtime_local_rewrite_error_log_value, runtime_local_rewrite_validate_resolved_bound_provider,
};
use prodex_provider_core::{ProviderId, RuntimeProviderBindingIdentity};

#[test]
fn local_rewrite_error_log_value_redacts_secret_like_chain() {
    let err = anyhow::anyhow!(
        "upstream failed\nAuthorization: Bearer local-rewrite-token\napi_key=local-rewrite-key"
    )
    .context("local rewrite upstream failed");
    let message = runtime_local_rewrite_error_log_value(&err);

    assert_eq!(message, "upstream_request_failed");
}

#[test]
fn governed_continuation_requires_the_exact_projected_binding_identity() {
    let bound = RuntimeProviderBindingIdentity::from_raw_key(
        ProviderId::Kiro,
        "synthetic-key-a",
        "https://kiro.example.com/v1",
        Some("governed-route"),
    )
    .unwrap();
    let other_key = RuntimeProviderBindingIdentity::from_raw_key(
        ProviderId::Kiro,
        "synthetic-key-b",
        "https://kiro.example.com/v1",
        Some("governed-route"),
    )
    .unwrap();
    let other_endpoint = RuntimeProviderBindingIdentity::from_raw_key(
        ProviderId::Kiro,
        "synthetic-key-a",
        "https://other.example.com/v1",
        Some("governed-route"),
    )
    .unwrap();

    assert!(
        runtime_local_rewrite_validate_resolved_bound_provider(
            Some(&bound),
            ProviderId::Kiro,
            Some(&bound),
        )
        .is_ok()
    );
    for selected in [&other_key, &other_endpoint] {
        assert!(
            runtime_local_rewrite_validate_resolved_bound_provider(
                Some(&bound),
                ProviderId::Kiro,
                Some(selected),
            )
            .is_err()
        );
    }
    assert!(
        runtime_local_rewrite_validate_resolved_bound_provider(
            Some(&bound),
            ProviderId::Gemini,
            Some(&bound),
        )
        .is_err()
    );
    assert!(
        runtime_local_rewrite_validate_resolved_bound_provider(
            None,
            ProviderId::Kiro,
            Some(&bound),
        )
        .is_err()
    );
    assert!(
        runtime_local_rewrite_validate_resolved_bound_provider(
            Some(&bound),
            ProviderId::Kiro,
            None,
        )
        .is_err()
    );
}
