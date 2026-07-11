use super::*;

#[test]
fn production_gateway_resolves_projected_credentials_and_rejects_raw_cli_secret() {
    let root = temp_dir("gateway-projected-secrets");
    let secret_root = root.join("projected");
    std::fs::create_dir_all(&secret_root).unwrap();
    for (name, value) in [
        ("gateway-token", "gateway-secret-value"),
        ("provider-key", "provider-secret-value"),
        ("postgres-url", "postgres://prodex@127.0.0.1/prodex"),
        ("admin-token", "admin-secret-value"),
        ("virtual-key", "virtual-secret-value"),
        ("sso-token", "sso-secret-value"),
        ("telemetry-token", "telemetry-secret-value"),
        ("webhook-token", "webhook-secret-value"),
    ] {
        let path = secret_root.join(name);
        std::fs::write(&path, value).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    let state = AppState::default();
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings {
        require_auth: Some(true),
        auth_token_ref: Some(secret_ref("gateway-token")),
        provider_api_key_ref: Some(secret_ref("provider-key")),
        ..Default::default()
    };
    policy.state.backend = Some("postgres".to_string());
    policy.state.postgres_url_ref = Some(secret_ref("postgres-url"));
    policy
        .admin_tokens
        .push(prodex_runtime_policy::RuntimePolicyGatewayAdminToken {
            name: "operations".to_string(),
            token_ref: Some(secret_ref("admin-token")),
            ..Default::default()
        });
    policy
        .virtual_keys
        .push(prodex_runtime_policy::RuntimePolicyGatewayVirtualKey {
            name: "tenant-client".to_string(),
            token_ref: Some(secret_ref("virtual-key")),
            ..Default::default()
        });
    policy.sso.proxy_token_ref = Some(secret_ref("sso-token"));
    policy.observability.http_bearer_token_ref = Some(secret_ref("telemetry-token"));
    policy.guardrails.webhook_bearer_token_ref = Some(secret_ref("webhook-token"));
    let secrets = prodex_runtime_policy::RuntimePolicySecretsSettings {
        production: true,
        projected_root: Some(secret_root),
        projected_provider: Some("external".to_string()),
        ..Default::default()
    };

    let config = resolve_gateway_launch_config_with_secrets(
        &paths,
        &state,
        &gateway_args(),
        &policy,
        &secrets,
    )
    .unwrap();
    assert!(config.auth_required);
    assert_eq!(config.admin_tokens.len(), 1);
    assert_eq!(config.virtual_keys.len(), 1);
    assert!(matches!(
        config.state_store,
        RuntimeGatewayStateStore::Postgres { .. }
    ));

    let mut raw_args = gateway_args();
    raw_args.auth_token = Some("raw-cli-secret".to_string());
    let error = match resolve_gateway_launch_config_with_secrets(
        &paths, &state, &raw_args, &policy, &secrets,
    ) {
        Err(error) => error,
        Ok(_) => panic!("raw production credential must be rejected"),
    };
    let message = format!("{error:#}");
    assert!(message.contains("exactly one secret source"));
    assert!(!message.contains("raw-cli-secret"));
}

fn secret_ref(name: &str) -> prodex_domain::SecretRef {
    prodex_domain::SecretRef::new("external", name, None::<String>)
}
