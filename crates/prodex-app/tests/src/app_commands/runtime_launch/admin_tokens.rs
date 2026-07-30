use super::*;

#[test]
fn gateway_admin_tokens_config_does_not_promote_data_plane_token() {
    let policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();

    let tokens = gateway_admin_tokens_config(Some("gateway-root-token"), &policy).unwrap();

    assert!(tokens.is_empty());
}

#[test]
fn gateway_admin_tokens_config_defaults_missing_role_to_viewer() {
    let _admin = TestEnvVarGuard::set("PRODEX_GATEWAY_ADMIN_TOKEN_TEST", "admin-secret");
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy
        .admin_tokens
        .push(prodex_runtime_policy::RuntimePolicyGatewayAdminToken {
            name: "scoped".to_string(),
            token_env: "PRODEX_GATEWAY_ADMIN_TOKEN_TEST".to_string(),
            ..Default::default()
        });

    let tokens = gateway_admin_tokens_config(None, &policy).unwrap();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].role, RuntimeGatewayAdminRole::Viewer);
}

#[test]
fn gateway_admin_tokens_config_rejects_duplicate_credential_values() {
    let _maker = TestEnvVarGuard::set("PRODEX_GATEWAY_ADMIN_MAKER_TEST", "shared-secret");
    let _checker = TestEnvVarGuard::set("PRODEX_GATEWAY_ADMIN_CHECKER_TEST", "shared-secret");
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    for (name, token_env) in [
        ("maker", "PRODEX_GATEWAY_ADMIN_MAKER_TEST"),
        ("checker", "PRODEX_GATEWAY_ADMIN_CHECKER_TEST"),
    ] {
        policy
            .admin_tokens
            .push(prodex_runtime_policy::RuntimePolicyGatewayAdminToken {
                name: name.to_string(),
                token_env: token_env.to_string(),
                role: Some("admin".to_string()),
                tenant_id: Some("00000000-0000-7000-8000-000000000001".to_string()),
                ..Default::default()
            });
    }

    let error = gateway_admin_tokens_config(None, &policy)
        .expect_err("duplicate resolved admin credentials must fail closed");

    assert!(error.to_string().contains("distinct credential values"));
}

#[test]
fn gateway_admin_tokens_config_rejects_unknown_role() {
    let _admin = TestEnvVarGuard::set("PRODEX_GATEWAY_ADMIN_TOKEN_TEST", "admin-secret");
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy
        .admin_tokens
        .push(prodex_runtime_policy::RuntimePolicyGatewayAdminToken {
            name: "scoped".to_string(),
            token_env: "PRODEX_GATEWAY_ADMIN_TOKEN_TEST".to_string(),
            role: Some("owner".to_string()),
            ..Default::default()
        });

    let err = gateway_admin_tokens_config(None, &policy).unwrap_err();

    assert!(
        err.to_string()
            .contains("gateway.admin_tokens role for \"scoped\"")
    );
}

#[test]
fn gateway_admin_tokens_config_rejects_invalid_key_prefix_scopes() {
    let _admin = TestEnvVarGuard::set("PRODEX_GATEWAY_ADMIN_TOKEN_TEST", "admin-secret");
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy
        .admin_tokens
        .push(prodex_runtime_policy::RuntimePolicyGatewayAdminToken {
            name: "scoped".to_string(),
            token_env: "PRODEX_GATEWAY_ADMIN_TOKEN_TEST".to_string(),
            allowed_key_prefixes: vec!["team-a-".to_string(), " team-b- ".to_string()],
            ..Default::default()
        });

    let err = gateway_admin_tokens_config(None, &policy).unwrap_err();

    assert!(
        err.to_string()
            .contains("gateway.admin_tokens allowed_key_prefixes for \"scoped\"")
    );
}

#[test]
fn gateway_admin_tokens_config_rejects_invalid_governance_scopes() {
    let _admin = TestEnvVarGuard::set("PRODEX_GATEWAY_ADMIN_TOKEN_TEST", "admin-secret");
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy
        .admin_tokens
        .push(prodex_runtime_policy::RuntimePolicyGatewayAdminToken {
            name: "scoped".to_string(),
            token_env: "PRODEX_GATEWAY_ADMIN_TOKEN_TEST".to_string(),
            tenant_id: Some(" tenant-a ".to_string()),
            team_id: Some("team-a".to_string()),
            project_id: Some(" project-a ".to_string()),
            ..Default::default()
        });

    let err = gateway_admin_tokens_config(None, &policy).unwrap_err();

    assert!(
        err.to_string()
            .contains("gateway.admin_tokens tenant_id for \"scoped\"")
    );
}

#[test]
fn gateway_admin_tokens_config_rejects_invalid_token_names() {
    let _admin = TestEnvVarGuard::set("PRODEX_GATEWAY_ADMIN_TOKEN_TEST", "admin-secret");
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy
        .admin_tokens
        .push(prodex_runtime_policy::RuntimePolicyGatewayAdminToken {
            name: " scoped ".to_string(),
            token_env: "PRODEX_GATEWAY_ADMIN_TOKEN_TEST".to_string(),
            ..Default::default()
        });

    let err = gateway_admin_tokens_config(None, &policy).unwrap_err();

    assert!(err.to_string().contains("gateway.admin_tokens name"));
}

#[test]
fn gateway_admin_tokens_config_rejects_invalid_token_env_refs() {
    let _admin = TestEnvVarGuard::set("PRODEX_GATEWAY_ADMIN_TOKEN_TEST", "admin-secret");
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy
        .admin_tokens
        .push(prodex_runtime_policy::RuntimePolicyGatewayAdminToken {
            name: "scoped".to_string(),
            token_env: " PRODEX_GATEWAY_ADMIN_TOKEN_TEST ".to_string(),
            ..Default::default()
        });

    let err = gateway_admin_tokens_config(None, &policy).unwrap_err();

    assert!(err.to_string().contains("gateway.admin_tokens token_env"));
}

#[test]
fn gateway_admin_tokens_config_rejects_missing_token_env_secret() {
    let _admin = TestEnvVarGuard::unset("PRODEX_GATEWAY_ADMIN_TOKEN_MISSING_TEST");
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy
        .admin_tokens
        .push(prodex_runtime_policy::RuntimePolicyGatewayAdminToken {
            name: "scoped".to_string(),
            token_env: "PRODEX_GATEWAY_ADMIN_TOKEN_MISSING_TEST".to_string(),
            ..Default::default()
        });

    let err = gateway_admin_tokens_config(None, &policy).unwrap_err();

    assert!(
        err.to_string()
            .contains("requires PRODEX_GATEWAY_ADMIN_TOKEN_MISSING_TEST")
    );
}

#[test]
fn gateway_admin_tokens_config_rejects_empty_token_env_secret() {
    let _admin = TestEnvVarGuard::set("PRODEX_GATEWAY_ADMIN_TOKEN_EMPTY_TEST", "");
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy
        .admin_tokens
        .push(prodex_runtime_policy::RuntimePolicyGatewayAdminToken {
            name: "scoped".to_string(),
            token_env: "PRODEX_GATEWAY_ADMIN_TOKEN_EMPTY_TEST".to_string(),
            ..Default::default()
        });

    let err = gateway_admin_tokens_config(None, &policy).unwrap_err();

    assert!(
        err.to_string()
            .contains("env PRODEX_GATEWAY_ADMIN_TOKEN_EMPTY_TEST cannot be empty")
    );
}

#[test]
fn gateway_admin_tokens_config_rejects_padded_token_env_secret() {
    let _admin = TestEnvVarGuard::set("PRODEX_GATEWAY_ADMIN_TOKEN_PADDED_TEST", " admin-secret ");
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy
        .admin_tokens
        .push(prodex_runtime_policy::RuntimePolicyGatewayAdminToken {
            name: "scoped".to_string(),
            token_env: "PRODEX_GATEWAY_ADMIN_TOKEN_PADDED_TEST".to_string(),
            ..Default::default()
        });

    let err = gateway_admin_tokens_config(None, &policy).unwrap_err();

    assert!(
        err.to_string()
            .contains("env PRODEX_GATEWAY_ADMIN_TOKEN_PADDED_TEST must not contain whitespace")
    );
}
