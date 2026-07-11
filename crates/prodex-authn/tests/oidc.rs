use prodex_authn::{
    AuthenticationError, AuthenticationErrorStatus, OidcRefreshRuntimeMode, OidcRefreshSource,
    TokenClaims, authenticate_oidc_claims, plan_authentication_error_response,
    plan_oidc_jwks_refresh,
};
use prodex_domain::{
    Audience, CredentialScope, ExplicitRoleMapper, Issuer, JwksCacheSnapshot, JwtAlgorithm,
    OidcValidationPolicy, PrincipalId, PrincipalKind, Role, RoleClaimError, TenantId,
    TokenValidationError,
};

fn policy() -> OidcValidationPolicy {
    OidcValidationPolicy::new(
        Issuer::new("https://issuer.example.com/").unwrap(),
        vec![Audience::new("prodex").unwrap()],
        vec![JwtAlgorithm::Rs256],
    )
    .unwrap()
}

fn jwks() -> JwksCacheSnapshot {
    JwksCacheSnapshot {
        fetched_at_unix_ms: 1_000,
        expires_at_unix_ms: 10_000,
        stale_until_unix_ms: 20_000,
        key_count: 2,
        last_refresh_error_at_unix_ms: None,
        retry_after_unix_ms: None,
    }
}

fn mapper() -> ExplicitRoleMapper {
    ExplicitRoleMapper::new([("operator", Role::Operator), ("admin", Role::Admin)])
}

fn claims(tenant_id: Option<TenantId>) -> TokenClaims {
    TokenClaims {
        issuer: Issuer::new("https://issuer.example.com").unwrap(),
        audience: Audience::new("prodex").unwrap(),
        algorithm: JwtAlgorithm::Rs256,
        key_id: "kid-1".to_string(),
        key_known: true,
        signature_verified: true,
        principal_id: PrincipalId::new(),
        tenant_id,
        principal_kind: PrincipalKind::User,
        credential_scope: CredentialScope::ControlPlane,
        role_claim: Some("operator".to_string()),
        expires_at_unix_ms: 9_000,
        not_before_unix_ms: Some(1_000),
    }
}

#[test]
fn authenticates_valid_oidc_claims_without_network_refresh() {
    let tenant_id = TenantId::new();
    let principal = authenticate_oidc_claims(
        &policy(),
        Some(&jwks()),
        &mapper(),
        claims(Some(tenant_id)),
        2_000,
    )
    .unwrap();

    assert_eq!(principal.tenant_id, Some(tenant_id));
    assert_eq!(principal.role, Role::Operator);
    assert_eq!(principal.credential_scope, CredentialScope::ControlPlane);
}

#[test]
fn rejects_request_path_authentication_when_jwks_must_be_fetched() {
    assert_eq!(
        authenticate_oidc_claims(
            &policy(),
            None,
            &mapper(),
            claims(Some(TenantId::new())),
            2_000
        ),
        Err(AuthenticationError::JwksRefreshRequired)
    );
}

#[test]
fn request_path_refresh_plan_rejects_network_discovery_or_jwks_fetch() {
    assert_eq!(
        plan_oidc_jwks_refresh(
            &policy(),
            Some("https://issuer.example.com/jwks.json"),
            None,
            2_000,
            OidcRefreshRuntimeMode::GatewayRequestPath,
        ),
        Err(AuthenticationError::JwksRefreshForbiddenOnRequestPath)
    );
}

#[test]
fn background_refresh_plan_selects_configured_jwks_or_discovery_url() {
    let direct = plan_oidc_jwks_refresh(
        &policy(),
        Some("https://issuer.example.com/jwks.json"),
        None,
        2_000,
        OidcRefreshRuntimeMode::ControlPlaneBackground,
    )
    .unwrap();
    assert_eq!(
        direct.decision,
        prodex_domain::JwksRefreshDecision::RefreshNow
    );
    assert_eq!(
        direct.source,
        Some(OidcRefreshSource::Jwks {
            url: "https://issuer.example.com/jwks.json".to_string()
        })
    );

    let discovery = plan_oidc_jwks_refresh(
        &policy(),
        None,
        None,
        2_000,
        OidcRefreshRuntimeMode::ControlPlaneBackground,
    )
    .unwrap();
    assert_eq!(
        discovery.source,
        Some(OidcRefreshSource::Discovery {
            url: "https://issuer.example.com/.well-known/openid-configuration".to_string()
        })
    );
}

#[test]
fn background_refresh_plan_rejects_invalid_jwks_url() {
    assert_eq!(
        plan_oidc_jwks_refresh(
            &policy(),
            Some("file:///tmp/jwks.json"),
            None,
            2_000,
            OidcRefreshRuntimeMode::ControlPlaneBackground,
        ),
        Err(AuthenticationError::InvalidJwksUrl)
    );
    assert_eq!(
        plan_oidc_jwks_refresh(
            &policy(),
            Some("http://issuer.example.com/jwks.json"),
            None,
            2_000,
            OidcRefreshRuntimeMode::ControlPlaneBackground,
        ),
        Err(AuthenticationError::InvalidJwksUrl)
    );
    assert_eq!(
        plan_oidc_jwks_refresh(
            &policy(),
            Some("https://issuer.example.com/jwks bad.json"),
            None,
            2_000,
            OidcRefreshRuntimeMode::ControlPlaneBackground,
        ),
        Err(AuthenticationError::InvalidJwksUrl)
    );
    assert_eq!(
        plan_oidc_jwks_refresh(
            &policy(),
            Some(" https://issuer.example.com/jwks.json "),
            None,
            2_000,
            OidcRefreshRuntimeMode::ControlPlaneBackground,
        ),
        Err(AuthenticationError::InvalidJwksUrl)
    );
    assert_eq!(
        plan_oidc_jwks_refresh(
            &policy(),
            Some("https://issuer.example.com/jwks☃.json"),
            None,
            2_000,
            OidcRefreshRuntimeMode::ControlPlaneBackground,
        ),
        Err(AuthenticationError::InvalidJwksUrl)
    );
    let overlong = format!("https://issuer.example.com/{}", "x".repeat(2_022));
    assert_eq!(
        plan_oidc_jwks_refresh(
            &policy(),
            Some(&overlong),
            None,
            2_000,
            OidcRefreshRuntimeMode::ControlPlaneBackground,
        ),
        Err(AuthenticationError::InvalidJwksUrl)
    );
    assert_eq!(
        plan_oidc_jwks_refresh(
            &policy(),
            Some("https://"),
            None,
            2_000,
            OidcRefreshRuntimeMode::ControlPlaneBackground,
        ),
        Err(AuthenticationError::InvalidJwksUrl)
    );
    assert_eq!(
        plan_oidc_jwks_refresh(
            &policy(),
            Some("https:///jwks.json"),
            None,
            2_000,
            OidcRefreshRuntimeMode::ControlPlaneBackground,
        ),
        Err(AuthenticationError::InvalidJwksUrl)
    );
}

#[test]
fn background_refresh_plan_rejects_cross_issuer_jwks_url() {
    assert_eq!(
        plan_oidc_jwks_refresh(
            &policy(),
            Some("https://evil.example.com/jwks.json"),
            None,
            2_000,
            OidcRefreshRuntimeMode::ControlPlaneBackground,
        ),
        Err(AuthenticationError::JwksUrlIssuerMismatch)
    );
}

#[test]
fn background_discovery_plan_rejects_issuer_without_https_host() {
    assert_eq!(
        Issuer::new("https://"),
        Err(prodex_domain::IdentityConfigError::InvalidIssuer)
    );
}

#[test]
fn refresh_plan_uses_fresh_or_backoff_cache_without_network_source() {
    let fresh = plan_oidc_jwks_refresh(
        &policy(),
        Some("https://issuer.example.com/jwks.json"),
        Some(&jwks()),
        2_000,
        OidcRefreshRuntimeMode::GatewayRequestPath,
    )
    .unwrap();
    assert_eq!(fresh.source, None);

    let backoff = JwksCacheSnapshot {
        expires_at_unix_ms: 1_500,
        stale_until_unix_ms: 10_000,
        retry_after_unix_ms: Some(5_000),
        ..jwks()
    };
    let plan = plan_oidc_jwks_refresh(
        &policy(),
        Some("https://issuer.example.com/jwks.json"),
        Some(&backoff),
        2_000,
        OidcRefreshRuntimeMode::GatewayRequestPath,
    )
    .unwrap();
    assert_eq!(plan.source, None);
}

#[test]
fn authenticates_with_stale_while_revalidate_jwks_without_request_path_fetch() {
    let stale_usable = JwksCacheSnapshot {
        expires_at_unix_ms: 1_500,
        stale_until_unix_ms: 10_000,
        retry_after_unix_ms: None,
        ..jwks()
    };

    let refresh = plan_oidc_jwks_refresh(
        &policy(),
        Some("https://issuer.example.com/jwks.json"),
        Some(&stale_usable),
        2_000,
        OidcRefreshRuntimeMode::GatewayRequestPath,
    )
    .unwrap();
    assert_eq!(
        refresh.decision,
        prodex_domain::JwksRefreshDecision::UseStaleWhileRevalidate
    );
    assert_eq!(refresh.source, None);

    assert!(
        authenticate_oidc_claims(
            &policy(),
            Some(&stale_usable),
            &mapper(),
            claims(Some(TenantId::new())),
            2_000
        )
        .is_ok()
    );
}

#[test]
fn rejects_unknown_key_unverified_signature_and_expired_token() {
    let mut unknown_key = claims(Some(TenantId::new()));
    unknown_key.key_known = false;
    assert_eq!(
        authenticate_oidc_claims(&policy(), Some(&jwks()), &mapper(), unknown_key, 2_000),
        Err(AuthenticationError::UnknownKeyId)
    );

    let mut empty_key_id = claims(Some(TenantId::new()));
    empty_key_id.key_id = "   ".to_string();
    assert_eq!(
        authenticate_oidc_claims(&policy(), Some(&jwks()), &mapper(), empty_key_id, 2_000),
        Err(AuthenticationError::UnknownKeyId)
    );

    for key_id in [" kid-1", "kid\n1", "kid☃", &"x".repeat(129)] {
        let mut malformed_key_id = claims(Some(TenantId::new()));
        malformed_key_id.key_id = key_id.to_string();
        assert_eq!(
            authenticate_oidc_claims(&policy(), Some(&jwks()), &mapper(), malformed_key_id, 2_000),
            Err(AuthenticationError::UnknownKeyId)
        );
    }

    let mut unverified = claims(Some(TenantId::new()));
    unverified.signature_verified = false;
    assert_eq!(
        authenticate_oidc_claims(&policy(), Some(&jwks()), &mapper(), unverified, 2_000),
        Err(AuthenticationError::SignatureNotVerified)
    );

    assert_eq!(
        authenticate_oidc_claims(
            &policy(),
            Some(&jwks()),
            &mapper(),
            claims(Some(TenantId::new())),
            9_000
        ),
        Err(AuthenticationError::TokenExpired)
    );

    let mut not_yet_valid = claims(Some(TenantId::new()));
    not_yet_valid.not_before_unix_ms = Some(3_000);
    assert_eq!(
        authenticate_oidc_claims(&policy(), Some(&jwks()), &mapper(), not_yet_valid, 2_000),
        Err(AuthenticationError::TokenNotYetValid)
    );
}

#[test]
fn rejects_malformed_claims_and_missing_tenant_or_role() {
    let mut wrong_issuer = claims(Some(TenantId::new()));
    wrong_issuer.issuer = Issuer::new("https://evil.example.com").unwrap();
    assert_eq!(
        authenticate_oidc_claims(&policy(), Some(&jwks()), &mapper(), wrong_issuer, 2_000),
        Err(AuthenticationError::Claims(
            TokenValidationError::IssuerMismatch
        ))
    );

    let mut wrong_audience = claims(Some(TenantId::new()));
    wrong_audience.audience = Audience::new("other-audience").unwrap();
    assert_eq!(
        authenticate_oidc_claims(&policy(), Some(&jwks()), &mapper(), wrong_audience, 2_000),
        Err(AuthenticationError::Claims(
            TokenValidationError::AudienceMismatch
        ))
    );

    let mut wrong_algorithm = claims(Some(TenantId::new()));
    wrong_algorithm.algorithm = JwtAlgorithm::EdDsa;
    assert_eq!(
        authenticate_oidc_claims(&policy(), Some(&jwks()), &mapper(), wrong_algorithm, 2_000),
        Err(AuthenticationError::Claims(
            TokenValidationError::AlgorithmNotAllowed
        ))
    );

    assert_eq!(
        authenticate_oidc_claims(&policy(), Some(&jwks()), &mapper(), claims(None), 2_000),
        Err(AuthenticationError::MissingTenant)
    );

    let mut missing_role = claims(Some(TenantId::new()));
    missing_role.role_claim = None;
    assert_eq!(
        authenticate_oidc_claims(&policy(), Some(&jwks()), &mapper(), missing_role, 2_000),
        Err(AuthenticationError::Role(RoleClaimError::Missing))
    );

    let mut unknown_role = claims(Some(TenantId::new()));
    unknown_role.role_claim = Some("super-admin-secret".to_string());
    assert_eq!(
        authenticate_oidc_claims(&policy(), Some(&jwks()), &mapper(), unknown_role, 2_000),
        Err(AuthenticationError::Role(RoleClaimError::Unknown))
    );
}

#[test]
fn allows_last_known_good_jwks_during_backoff_but_rejects_unusable_stale_cache() {
    let usable_lkg = JwksCacheSnapshot {
        expires_at_unix_ms: 1_500,
        stale_until_unix_ms: 10_000,
        retry_after_unix_ms: Some(5_000),
        ..jwks()
    };
    assert!(
        authenticate_oidc_claims(
            &policy(),
            Some(&usable_lkg),
            &mapper(),
            claims(Some(TenantId::new())),
            2_000
        )
        .is_ok()
    );

    let unusable = JwksCacheSnapshot {
        expires_at_unix_ms: 1_500,
        stale_until_unix_ms: 1_999,
        retry_after_unix_ms: Some(5_000),
        ..jwks()
    };
    assert_eq!(
        authenticate_oidc_claims(
            &policy(),
            Some(&unusable),
            &mapper(),
            claims(Some(TenantId::new())),
            2_000
        ),
        Err(AuthenticationError::JwksUnavailable)
    );
}

#[test]
fn authentication_error_responses_are_stable_and_redacted() {
    let invalid = plan_authentication_error_response(&AuthenticationError::UnknownKeyId);
    assert_eq!(invalid.status, AuthenticationErrorStatus::Unauthorized);
    assert_eq!(invalid.code, "invalid_token");
    assert_eq!(invalid.message, "token is invalid");
    assert!(!invalid.message.contains("kid"));

    let unavailable = plan_authentication_error_response(&AuthenticationError::JwksRefreshRequired);
    assert_eq!(
        unavailable.status,
        AuthenticationErrorStatus::ServiceUnavailable
    );
    assert_eq!(unavailable.code, "authentication_temporarily_unavailable");
    assert!(!unavailable.message.contains("JWKS"));

    let forbidden =
        plan_authentication_error_response(&AuthenticationError::JwksRefreshForbiddenOnRequestPath);
    assert_eq!(
        forbidden.status,
        AuthenticationErrorStatus::ServiceUnavailable
    );
    assert_eq!(forbidden.code, "authentication_temporarily_unavailable");
    assert!(!forbidden.message.contains("JWKS"));
    assert!(!forbidden.message.contains("request path"));

    let invalid_url = plan_authentication_error_response(&AuthenticationError::InvalidJwksUrl);
    assert_eq!(
        invalid_url.status,
        AuthenticationErrorStatus::ServiceUnavailable
    );
    assert_eq!(invalid_url.code, "authentication_temporarily_unavailable");
    assert!(!invalid_url.message.contains("jwks"));
    assert!(!invalid_url.message.contains("file:"));

    let issuer_mismatch =
        plan_authentication_error_response(&AuthenticationError::JwksUrlIssuerMismatch);
    assert_eq!(
        issuer_mismatch.status,
        AuthenticationErrorStatus::ServiceUnavailable
    );
    assert_eq!(
        issuer_mismatch.code,
        "authentication_temporarily_unavailable"
    );
    assert!(!issuer_mismatch.message.contains("issuer.example.com"));
    assert!(!issuer_mismatch.message.contains("evil.example.com"));

    let tenant = plan_authentication_error_response(&AuthenticationError::MissingTenant);
    assert_eq!(tenant.status, AuthenticationErrorStatus::Unauthorized);
    assert_eq!(tenant.code, "tenant_required");

    let role =
        plan_authentication_error_response(&AuthenticationError::Role(RoleClaimError::Missing));
    assert_eq!(role.status, AuthenticationErrorStatus::Unauthorized);
    assert_eq!(role.code, "role_not_authorized");
    assert!(!role.message.contains("Admin"));

    let unknown_role =
        plan_authentication_error_response(&AuthenticationError::Role(RoleClaimError::Unknown));
    assert_eq!(unknown_role.status, AuthenticationErrorStatus::Unauthorized);
    assert_eq!(unknown_role.code, "role_not_authorized");
    assert!(!unknown_role.message.contains("super-admin-secret"));
    assert!(!unknown_role.message.contains("Admin"));
}
