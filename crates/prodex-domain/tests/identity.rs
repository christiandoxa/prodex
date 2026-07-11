use prodex_domain::{
    Audience, IdentityConfigError, IdentityErrorStatus, Issuer, JwksCacheSnapshot,
    JwksRefreshDecision, JwtAlgorithm, OidcValidationPolicy, TokenValidationError,
    evaluate_jwks_refresh, plan_identity_config_error_response,
    plan_jwks_refresh_decision_error_response, plan_token_validation_error_response,
};

fn policy() -> OidcValidationPolicy {
    OidcValidationPolicy::new(
        Issuer::new("https://idp.example.com/").unwrap(),
        vec![Audience::new("prodex-control-plane").unwrap()],
        vec![JwtAlgorithm::Rs256],
    )
    .unwrap()
}

#[test]
fn oidc_policy_requires_issuer_audience_and_algorithm_allowlist() {
    assert_eq!(Issuer::new(""), Err(IdentityConfigError::EmptyIssuer));
    assert_eq!(Issuer::new(" "), Err(IdentityConfigError::InvalidIssuer));
    assert_eq!(
        Issuer::new("https://"),
        Err(IdentityConfigError::InvalidIssuer)
    );
    assert_eq!(
        Issuer::new("https:///issuer"),
        Err(IdentityConfigError::InvalidIssuer)
    );
    assert_eq!(
        Issuer::new("http://idp.example.com"),
        Err(IdentityConfigError::InvalidIssuer)
    );
    assert_eq!(
        Issuer::new("idp.example.com"),
        Err(IdentityConfigError::InvalidIssuer)
    );
    assert_eq!(
        Issuer::new("https://idp example.com"),
        Err(IdentityConfigError::InvalidIssuer)
    );
    assert_eq!(
        Issuer::new(" https://idp.example.com"),
        Err(IdentityConfigError::InvalidIssuer)
    );
    assert_eq!(
        Issuer::new("https://user:pass@idp.example.com"),
        Err(IdentityConfigError::InvalidIssuer)
    );
    assert_eq!(
        Issuer::new("https://idp.example.com:443").unwrap().as_str(),
        "https://idp.example.com:443"
    );
    assert_eq!(Audience::new(""), Err(IdentityConfigError::EmptyAudience));
    assert_eq!(
        Audience::new(" "),
        Err(IdentityConfigError::InvalidAudience)
    );
    assert_eq!(
        Audience::new("prodex control-plane"),
        Err(IdentityConfigError::InvalidAudience)
    );
    assert_eq!(
        Audience::new(" prodex-control-plane"),
        Err(IdentityConfigError::InvalidAudience)
    );
    assert_eq!(
        Audience::new("prodex\ncontrol-plane"),
        Err(IdentityConfigError::InvalidAudience)
    );
    assert_eq!(
        Audience::new("prodex-contrôle-plane"),
        Err(IdentityConfigError::InvalidAudience)
    );
    assert_eq!(
        OidcValidationPolicy::new(
            Issuer::new("https://idp.example.com").unwrap(),
            vec![],
            vec![JwtAlgorithm::Rs256]
        ),
        Err(IdentityConfigError::NoAudience)
    );
    assert_eq!(
        OidcValidationPolicy::new(
            Issuer::new("https://idp.example.com").unwrap(),
            vec![Audience::new("prodex").unwrap()],
            vec![],
        ),
        Err(IdentityConfigError::NoAllowedAlgorithm)
    );
}

#[test]
fn issuer_debug_output_is_stable_and_redacted() {
    let issuer = Issuer::new("https://idp.example.com/").unwrap();

    let rendered = format!("{issuer:?}");
    assert!(!rendered.contains("idp.example.com"));
    assert_eq!(rendered, "Issuer(\"<redacted>\")");
}

#[test]
fn audience_debug_output_is_stable_and_redacted() {
    let audience = Audience::new("prodex-control-plane").unwrap();

    let rendered = format!("{audience:?}");
    assert!(!rendered.contains("prodex-control-plane"));
    assert_eq!(rendered, "Audience(\"<redacted>\")");
}

#[test]
fn oidc_policy_debug_output_is_stable_and_redacted() {
    let policy = policy();

    let rendered = format!("{policy:?}");
    assert!(!rendered.contains("idp.example.com"));
    assert!(!rendered.contains("prodex-control-plane"));
    assert!(rendered.contains("allowed_algorithms: [Rs256]"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn oidc_policy_accepts_matching_claims_and_allowlisted_algorithm() {
    assert_eq!(
        policy().validate_claims(
            &Issuer::new("https://idp.example.com").unwrap(),
            &Audience::new("prodex-control-plane").unwrap(),
            JwtAlgorithm::Rs256,
        ),
        Ok(())
    );
}

#[test]
fn oidc_policy_rejects_issuer_audience_and_algorithm_mismatch() {
    assert_eq!(
        policy().validate_claims(
            &Issuer::new("https://other.example.com").unwrap(),
            &Audience::new("prodex-control-plane").unwrap(),
            JwtAlgorithm::Rs256,
        ),
        Err(TokenValidationError::IssuerMismatch)
    );
    assert_eq!(
        policy().validate_claims(
            &Issuer::new("https://idp.example.com").unwrap(),
            &Audience::new("other-audience").unwrap(),
            JwtAlgorithm::Rs256,
        ),
        Err(TokenValidationError::AudienceMismatch)
    );
    assert_eq!(
        policy().validate_claims(
            &Issuer::new("https://idp.example.com").unwrap(),
            &Audience::new("prodex-control-plane").unwrap(),
            JwtAlgorithm::EdDsa,
        ),
        Err(TokenValidationError::AlgorithmNotAllowed)
    );
}

#[test]
fn jwks_cache_reports_fresh_refresh_and_last_known_good_windows() {
    let cache = JwksCacheSnapshot {
        fetched_at_unix_ms: 1_000,
        expires_at_unix_ms: 10_000,
        stale_until_unix_ms: 20_000,
        key_count: 2,
        last_refresh_error_at_unix_ms: None,
        retry_after_unix_ms: None,
    };

    assert!(cache.is_fresh_at(9_999));
    assert!(!cache.should_refresh_at(9_999));
    assert!(!cache.is_fresh_at(10_000));
    assert!(cache.should_refresh_at(10_000));
    assert!(cache.is_usable_last_known_good_at(19_999));
    assert!(!cache.is_usable_last_known_good_at(20_000));
}

#[test]
fn empty_jwks_cache_is_never_usable() {
    let cache = JwksCacheSnapshot {
        fetched_at_unix_ms: 1_000,
        expires_at_unix_ms: 10_000,
        stale_until_unix_ms: 20_000,
        key_count: 0,
        last_refresh_error_at_unix_ms: None,
        retry_after_unix_ms: None,
    };

    assert!(!cache.is_fresh_at(2_000));
    assert!(!cache.is_usable_last_known_good_at(2_000));
}

#[test]
fn jwks_cache_snapshot_debug_output_is_stable_and_redacted() {
    let cache = JwksCacheSnapshot {
        fetched_at_unix_ms: 1_000,
        expires_at_unix_ms: 10_000,
        stale_until_unix_ms: 20_000,
        key_count: 2,
        last_refresh_error_at_unix_ms: Some(9_000),
        retry_after_unix_ms: Some(15_000),
    };

    let rendered = format!("{cache:?}");
    for sensitive in ["1000", "10000", "20000", "9000", "15000", "key_count"] {
        assert!(
            !rendered.contains(sensitive),
            "jwks cache debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("has_keys: true"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn jwks_refresh_decision_covers_missing_fresh_stale_backoff_and_unavailable() {
    assert_eq!(
        evaluate_jwks_refresh(None, 1_000),
        JwksRefreshDecision::RefreshNow
    );

    let fresh = JwksCacheSnapshot {
        fetched_at_unix_ms: 1_000,
        expires_at_unix_ms: 10_000,
        stale_until_unix_ms: 20_000,
        key_count: 1,
        last_refresh_error_at_unix_ms: None,
        retry_after_unix_ms: None,
    };
    assert_eq!(
        evaluate_jwks_refresh(Some(&fresh), 9_999),
        JwksRefreshDecision::UseFresh
    );
    assert_eq!(
        evaluate_jwks_refresh(Some(&fresh), 10_000),
        JwksRefreshDecision::UseStaleWhileRevalidate
    );

    let backoff = JwksCacheSnapshot {
        last_refresh_error_at_unix_ms: Some(10_000),
        retry_after_unix_ms: Some(15_000),
        ..fresh.clone()
    };
    assert_eq!(
        evaluate_jwks_refresh(Some(&backoff), 12_000),
        JwksRefreshDecision::UseLastKnownGoodDuringBackoff
    );
    assert_eq!(
        evaluate_jwks_refresh(Some(&backoff), 15_000),
        JwksRefreshDecision::UseStaleWhileRevalidate
    );

    let expired = JwksCacheSnapshot {
        stale_until_unix_ms: 12_000,
        ..backoff
    };
    assert_eq!(
        evaluate_jwks_refresh(Some(&expired), 13_000),
        JwksRefreshDecision::Unavailable
    );
}

#[test]
fn identity_error_responses_are_stable_and_redacted() {
    assert_eq!(
        IdentityConfigError::InvalidIssuer.to_string(),
        "identity configuration is invalid"
    );
    assert_eq!(
        TokenValidationError::IssuerMismatch.to_string(),
        "token validation failed"
    );
    for rendered in [
        IdentityConfigError::EmptyAudience.to_string(),
        TokenValidationError::AudienceMismatch.to_string(),
        TokenValidationError::AlgorithmNotAllowed.to_string(),
    ] {
        assert!(!rendered.contains("issuer"));
        assert!(!rendered.contains("audience"));
        assert!(!rendered.contains("algorithm"));
        assert!(!rendered.contains("JWT"));
    }

    let config = plan_identity_config_error_response(&IdentityConfigError::NoAllowedAlgorithm);
    assert_eq!(config.status, IdentityErrorStatus::InvalidRequest);
    assert_eq!(config.code, "identity_algorithm_allowlist_required");
    assert_eq!(config.message, "identity configuration is invalid");

    let token = plan_token_validation_error_response(&TokenValidationError::AudienceMismatch);
    assert_eq!(token.status, IdentityErrorStatus::Unauthorized);
    assert_eq!(token.code, "token_audience_invalid");
    assert_eq!(token.message, "token validation failed");

    let unavailable =
        plan_jwks_refresh_decision_error_response(JwksRefreshDecision::Unavailable).unwrap();
    assert_eq!(unavailable.status, IdentityErrorStatus::ServiceUnavailable);
    assert_eq!(unavailable.code, "jwks_cache_unavailable");
    assert_eq!(unavailable.message, "identity key cache is unavailable");

    assert!(
        plan_jwks_refresh_decision_error_response(
            JwksRefreshDecision::UseLastKnownGoodDuringBackoff,
        )
        .is_none()
    );

    let rendered = format!("{config:?} {token:?} {unavailable:?}");
    for sensitive in [
        "https://idp.example.com",
        "prodex-control-plane",
        "kid",
        "key_count",
        "retry_after",
        "last_refresh_error",
        "stale_until",
        "expires_at",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "identity response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}
