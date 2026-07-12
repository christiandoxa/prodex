use super::*;

#[test]
fn sso_role_resolution_never_defaults_missing_or_unknown_to_admin() {
    let scim_admin = RuntimeGatewayScimUser {
        id: "user-1".to_string(),
        user_name: "alice@example.com".to_string(),
        external_id: None,
        display_name: None,
        active: true,
        role: Some("admin".to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: Some("user-1".to_string()),
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
        created_at_epoch: 1,
        updated_at_epoch: 1,
    };

    assert_eq!(
        runtime_gateway_sso_resolved_role(None, None),
        RuntimeGatewayAdminRole::Viewer
    );
    assert_eq!(
        runtime_gateway_sso_resolved_role(None, Some(&scim_admin)),
        RuntimeGatewayAdminRole::Admin
    );
    assert_eq!(
        runtime_gateway_sso_resolved_role(Some("not-admin"), Some(&scim_admin)),
        RuntimeGatewayAdminRole::Viewer
    );
    assert_eq!(
        runtime_gateway_sso_resolved_role(Some(" admin "), Some(&scim_admin)),
        RuntimeGatewayAdminRole::Viewer
    );
}

#[test]
fn oidc_malformed_role_claim_does_not_fall_back_to_scim_admin() {
    let scim_admin = RuntimeGatewayScimUser {
        id: "user-1".to_string(),
        user_name: "alice@example.com".to_string(),
        external_id: None,
        display_name: None,
        active: true,
        role: Some("admin".to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: Some("user-1".to_string()),
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
        created_at_epoch: 1,
        updated_at_epoch: 1,
    };
    let malformed_claims =
        BTreeMap::from([("prodex_role".to_string(), serde_json::json!(" admin "))]);

    assert_eq!(
        runtime_gateway_sso_resolved_role(
            runtime_gateway_oidc_role_claim_string(&malformed_claims, "prodex_role").as_deref(),
            Some(&scim_admin),
        ),
        RuntimeGatewayAdminRole::Viewer
    );

    assert_eq!(
        runtime_gateway_sso_resolved_role(
            runtime_gateway_oidc_role_claim_string(&BTreeMap::new(), "prodex_role").as_deref(),
            Some(&scim_admin),
        ),
        RuntimeGatewayAdminRole::Admin
    );
}

#[test]
fn claimed_tenant_mismatch_does_not_reuse_scim_admin_role() {
    let user = RuntimeGatewayScimUser {
        id: "user-1".to_string(),
        user_name: "alice@example.com".to_string(),
        external_id: None,
        display_name: None,
        active: true,
        role: Some("admin".to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: Some("user-1".to_string()),
        budget_id: None,
        allowed_key_prefixes: vec!["tenant-a-".to_string()],
        created_at_epoch: 1,
        updated_at_epoch: 1,
    };

    let matched =
        runtime_gateway_scim_user_for_claimed_tenant(Some(user.clone()), Some("tenant-a"));
    assert_eq!(
        runtime_gateway_sso_resolved_role(None, matched.as_ref()),
        RuntimeGatewayAdminRole::Admin
    );

    let mismatched = runtime_gateway_scim_user_for_claimed_tenant(Some(user), Some("tenant-b"));
    assert!(mismatched.is_none());
    assert_eq!(
        runtime_gateway_sso_resolved_role(None, mismatched.as_ref()),
        RuntimeGatewayAdminRole::Viewer
    );
}

#[test]
fn missing_tenant_claim_may_still_use_scim_tenant_fallback() {
    let user = RuntimeGatewayScimUser {
        id: "user-1".to_string(),
        user_name: "alice@example.com".to_string(),
        external_id: None,
        display_name: None,
        active: true,
        role: Some("viewer".to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: Some("user-1".to_string()),
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
        created_at_epoch: 1,
        updated_at_epoch: 1,
    };

    assert!(
        runtime_gateway_scim_user_for_claimed_tenant(Some(user), None)
            .is_some_and(|user| user.tenant_id.as_deref() == Some("tenant-a"))
    );
}

#[test]
fn sso_proxy_token_matches_exactly_without_trimming() {
    let token_hash = runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("sso-proxy-token");

    assert!(runtime_gateway_sso_proxy_token_matches(
        &token_hash,
        "sso-proxy-token"
    ));
    assert!(!runtime_gateway_sso_proxy_token_matches(
        &token_hash,
        " sso-proxy-token "
    ));
}

#[test]
fn sso_user_names_match_exactly_without_trimming() {
    assert_eq!(runtime_gateway_sso_user_name(None), None);
    assert_eq!(
        runtime_gateway_sso_user_name(Some("alice@example.com")),
        Some("alice@example.com")
    );
    assert_eq!(
        runtime_gateway_sso_user_name(Some(" alice@example.com ")),
        None
    );
    assert_eq!(runtime_gateway_sso_user_name(Some("")), None);
}

#[test]
fn oidc_principal_claims_match_exactly_without_trimming() {
    let claims = BTreeMap::from([
        (
            "email".to_string(),
            serde_json::json!(" alice@example.com "),
        ),
        (
            "preferred_username".to_string(),
            serde_json::json!("bob@example.com"),
        ),
    ]);

    assert_eq!(runtime_gateway_oidc_claim_string(&claims, "email"), None);
    assert_eq!(
        runtime_gateway_oidc_claim_string(&claims, "preferred_username"),
        Some("bob@example.com".to_string())
    );
}

#[test]
fn oidc_configured_user_claim_must_be_exact_when_present() {
    let config = RuntimeGatewayOidcConfig {
        issuer: "https://idp.example".to_string(),
        audience: "prodex-gateway".to_string(),
        jwks_url: None,
        jwks_origin_allowlist: Vec::new(),
        user_claim: "prodex_user".to_string(),
        role_claim: "prodex_role".to_string(),
        tenant_claim: "prodex_tenant".to_string(),
        key_prefixes_claim: "prodex_key_prefixes".to_string(),
    };
    let malformed_configured_claim = BTreeMap::from([
        (
            "prodex_user".to_string(),
            serde_json::json!(" alice@example.com "),
        ),
        (
            "email".to_string(),
            serde_json::json!("fallback@example.com"),
        ),
    ]);

    assert_eq!(
        runtime_gateway_oidc_admin_name(&malformed_configured_claim, &config),
        None
    );

    let missing_configured_claim = BTreeMap::from([(
        "email".to_string(),
        serde_json::json!("fallback@example.com"),
    )]);
    assert_eq!(
        runtime_gateway_oidc_admin_name(&missing_configured_claim, &config),
        Some("fallback@example.com".to_string())
    );

    let malformed_first_fallback = BTreeMap::from([
        (
            "email".to_string(),
            serde_json::json!(" fallback@example.com "),
        ),
        (
            "preferred_username".to_string(),
            serde_json::json!("preferred@example.com"),
        ),
    ]);
    assert_eq!(
        runtime_gateway_oidc_admin_name(&malformed_first_fallback, &config),
        None
    );
}

#[test]
fn sso_key_prefixes_reject_empty_or_whitespace_scope_parts() {
    assert_eq!(
        runtime_gateway_parse_sso_prefixes("team-a-,team-b-"),
        Some(vec!["team-a-".to_string(), "team-b-".to_string()])
    );
    assert_eq!(runtime_gateway_parse_sso_prefixes(""), None);
    assert_eq!(runtime_gateway_parse_sso_prefixes("team-a-, team-b-"), None);
    assert_eq!(runtime_gateway_parse_sso_prefixes("team-a-,"), None);
}

#[test]
fn oidc_key_prefix_claim_rejects_whitespace_scope_parts() {
    let claims = BTreeMap::from([(
        "prodex_key_prefixes".to_string(),
        serde_json::json!(["team-a-", " team-b-"]),
    )]);

    assert!(runtime_gateway_oidc_claim_string_vec(&claims, "prodex_key_prefixes").is_err());
}
