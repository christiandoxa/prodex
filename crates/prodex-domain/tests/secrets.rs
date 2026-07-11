use prodex_domain::{
    SecretErrorStatus, SecretMaterial, SecretProvider, SecretProviderDescriptor, SecretPurpose,
    SecretRef, SecretResolutionError, SecretResolutionRequest, SecretRotationPolicy,
    SecretRotationPolicyError, SecretRotationStatus, plan_secret_resolution_error_response,
    plan_secret_rotation_policy_error_response,
};

#[test]
fn secret_ref_debug_and_display_do_not_expose_secret_name_or_version() {
    let secret = SecretRef::new("vault", "prod/openai/root-token", Some("v42"));

    let debug = format!("{secret:?}");
    let display = secret.to_string();

    assert!(!debug.contains("vault"));
    assert!(!debug.contains("prod/openai/root-token"));
    assert!(!debug.contains("v42"));
    assert_eq!(display, "<redacted-secret-ref>");
}

#[test]
fn secret_ref_keeps_structured_reference_for_secret_provider_lookup() {
    let secret = SecretRef::new("external-secrets", "provider-credential", None::<String>);

    assert_eq!(secret.provider(), "external-secrets");
    assert_eq!(secret.name(), "provider-credential");
    assert_eq!(secret.version(), None);
    assert!(secret.is_well_formed());
}

#[test]
fn secret_ref_well_formed_rejects_empty_or_whitespace_parts() {
    assert!(!SecretRef::new("", "providers/openai", None::<String>).is_well_formed());
    assert!(!SecretRef::new("vault", "", None::<String>).is_well_formed());
    assert!(!SecretRef::new("vault prod", "providers/openai", None::<String>).is_well_formed());
    assert!(!SecretRef::new("vault", "providers/openai key", None::<String>).is_well_formed());
    assert!(!SecretRef::new("vault", "providers/openai", Some(" ")).is_well_formed());
}

#[test]
fn secret_ref_well_formed_rejects_non_printable_or_overlong_parts() {
    let overlong = "x".repeat(129);

    assert!(!SecretRef::new("vault\nprod", "providers/openai", None::<String>).is_well_formed());
    assert!(!SecretRef::new("välut", "providers/openai", None::<String>).is_well_formed());
    assert!(!SecretRef::new("vault", "providers/openai\u{7f}", None::<String>).is_well_formed());
    assert!(!SecretRef::new("vault", "providers/openai", Some("v☃")).is_well_formed());
    assert!(
        !SecretRef::new(overlong.as_str(), "providers/openai", None::<String>).is_well_formed()
    );
    assert!(!SecretRef::new("vault", overlong.as_str(), None::<String>).is_well_formed());
    assert!(!SecretRef::new("vault", "providers/openai", Some(overlong.as_str())).is_well_formed());
}

#[test]
fn secret_material_debug_display_do_not_expose_value_or_version() {
    let material = SecretMaterial::new("super-secret-token", Some("v7"));

    material.with_exposed_secret(|secret| assert_eq!(secret, b"super-secret-token"));
    assert_eq!(material.version(), Some("v7"));
    assert!(!format!("{material:?}").contains("super-secret-token"));
    assert!(!format!("{material:?}").contains("v7"));
    assert_eq!(material.to_string(), "<redacted-secret>");

    fn requires_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>() {}
    requires_zeroize_on_drop::<SecretMaterial>();
}

#[test]
fn secret_provider_contract_resolves_reference_by_purpose_without_raw_domain_secret() {
    struct StaticProvider;

    impl SecretProvider for StaticProvider {
        fn descriptor(&self) -> SecretProviderDescriptor {
            SecretProviderDescriptor::external("vault")
        }

        fn resolve(
            &self,
            request: &SecretResolutionRequest,
        ) -> Result<SecretMaterial, SecretResolutionError> {
            assert_eq!(request.reference.provider(), "vault");
            assert_eq!(request.purpose, SecretPurpose::ProviderCredential);
            Ok(SecretMaterial::new("resolved-token", Some("v2")))
        }
    }

    let provider = StaticProvider;
    let descriptor = provider.descriptor();
    assert!(descriptor.supports_rotation_without_restart);
    let rendered_descriptor = format!("{descriptor:?}");
    assert!(!rendered_descriptor.contains("vault"));
    assert!(rendered_descriptor.contains("kind: ExternalSecretManager"));
    assert!(rendered_descriptor.contains("name: \"<redacted>\""));

    let material = provider
        .resolve(&SecretResolutionRequest::new(
            SecretRef::new("vault", "providers/openai", Some("v2")),
            SecretPurpose::ProviderCredential,
        ))
        .unwrap();

    material.with_exposed_secret(|secret| assert_eq!(secret, b"resolved-token"));
    assert_eq!(material.version(), Some("v2"));
}

#[test]
fn secret_request_and_rotation_debug_output_is_stable_and_redacted() {
    let request = SecretResolutionRequest::new(
        SecretRef::new("vault", "providers/openai-root-token", Some("v42")),
        SecretPurpose::ProviderCredential,
    );
    let status =
        SecretRotationStatus::active_only(SecretRef::new("vault", "gateway/token-v1", Some("v1")))
            .rotate_to(
                SecretRef::new("vault", "gateway/token-v2", Some("v2")),
                Some(1_900_000_000),
            );

    let rendered = format!("{request:?} {status:?}");
    for sensitive in [
        "vault",
        "providers/openai-root-token",
        "gateway/token-v1",
        "gateway/token-v2",
        "v42",
        "v1",
        "v2",
        "1900000000",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "secret debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("purpose: ProviderCredential"));
    assert!(rendered.contains("has_previous: true"));
    assert!(rendered.contains("next_refresh_after_epoch_seconds: Some(\"<redacted>\")"));
}

#[test]
fn secret_rotation_policy_requires_bounded_overlap_and_audit() {
    let policy = SecretRotationPolicy::short_lived(600);

    assert_eq!(policy.overlap_seconds, 60);
    assert!(policy.requires_audit_event);
    assert_eq!(policy.validate(), Ok(()));
    let rendered = format!("{policy:?}");
    assert!(!rendered.contains("600"));
    assert!(!rendered.contains("60"));
    assert!(rendered.contains("requires_audit_event: true"));
    assert!(rendered.contains("max_age_seconds: \"<redacted>\""));
    assert_eq!(
        SecretRotationPolicy {
            max_age_seconds: 0,
            overlap_seconds: 0,
            requires_audit_event: true,
        }
        .validate(),
        Err(SecretRotationPolicyError::ZeroMaxAge)
    );
    assert_eq!(
        SecretRotationPolicy {
            max_age_seconds: 10,
            overlap_seconds: 10,
            requires_audit_event: true,
        }
        .validate(),
        Err(SecretRotationPolicyError::OverlapNotShorterThanMaxAge)
    );
}

#[test]
fn secret_rotation_status_tracks_previous_without_exposing_names_in_display() {
    let initial = SecretRotationStatus::active_only(SecretRef::new(
        "external-secrets",
        "gateway/token-v1",
        Some("v1"),
    ));
    let rotated = initial.rotate_to(
        SecretRef::new("external-secrets", "gateway/token-v2", Some("v2")),
        Some(1_900_000_000),
    );

    assert_eq!(rotated.previous.as_ref().unwrap().version(), Some("v1"));
    assert_eq!(rotated.active.version(), Some("v2"));
    assert_eq!(
        rotated.next_refresh_after_epoch_seconds,
        Some(1_900_000_000)
    );
    assert_eq!(rotated.active.to_string(), "<redacted-secret-ref>");
}

#[test]
fn secret_error_responses_are_stable_and_redacted() {
    let denied = plan_secret_resolution_error_response(&SecretResolutionError::PermissionDenied);
    assert_eq!(denied.status, SecretErrorStatus::Forbidden);
    assert_eq!(denied.code, "secret_permission_denied");
    assert_eq!(denied.message, "secret could not be resolved");

    let unavailable =
        plan_secret_resolution_error_response(&SecretResolutionError::ProviderUnavailable);
    assert_eq!(unavailable.status, SecretErrorStatus::ServiceUnavailable);
    assert_eq!(unavailable.code, "secret_provider_unavailable");
    assert_eq!(unavailable.message, "secret provider is unavailable");

    let stale = plan_secret_resolution_error_response(&SecretResolutionError::StaleVersion);
    assert_eq!(stale.status, SecretErrorStatus::Conflict);
    assert_eq!(stale.code, "secret_version_stale");

    let rotation = plan_secret_rotation_policy_error_response(
        &SecretRotationPolicyError::OverlapNotShorterThanMaxAge,
    );
    assert_eq!(rotation.status, SecretErrorStatus::InvalidRequest);
    assert_eq!(rotation.code, "secret_rotation_overlap_invalid");
    assert_eq!(rotation.message, "secret rotation policy is invalid");

    let rendered = format!("{denied:?} {unavailable:?} {stale:?} {rotation:?}");
    for sensitive in [
        "prod/openai/root-token",
        "providers/openai",
        "gateway/token-v1",
        "gateway/token-v2",
        "v42",
        "v2",
        "v1",
        "v7",
        "super-secret-token",
        "resolved-token",
        "1900000000",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "secret response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}
