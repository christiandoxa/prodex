use super::*;

#[test]
fn production_gateway_resolves_projected_credentials_and_rejects_raw_cli_secret() {
    let root = temp_dir("gateway-projected-secrets");
    let secret_root = root.join("projected");
    std::fs::create_dir_all(&secret_root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&secret_root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    for (name, value) in [
        ("gateway-token", "gateway-secret-value"),
        ("provider-key", "provider-secret-value"),
        ("postgres-url", "postgres://prodex@127.0.0.1/prodex"),
        ("redis-url", "rediss://redis.internal/0"),
        ("admin-token", "admin-secret-value"),
        ("virtual-key", "virtual-secret-value"),
        ("sso-token", "sso-secret-value"),
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
    policy.state.redis_url_ref = Some(secret_ref("redis-url"));
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
    let RuntimeLocalRewriteProviderOptions::ProjectedCredential { credential, .. } =
        &config.provider_options
    else {
        panic!("production provider credential should remain deferred");
    };
    assert_eq!(credential.reference(), &secret_ref("provider-key"));
    assert_eq!(config.admin_tokens.len(), 1);
    assert_eq!(config.virtual_keys.len(), 1);
    assert!(config.observability.http_bearer_token.is_some());
    assert!(config.guardrail_webhook.bearer_token.is_some());
    let outbound_debug = format!("{:?} {:?}", config.observability, config.guardrail_webhook);
    assert!(!outbound_debug.contains("telemetry-token"));
    assert!(!outbound_debug.contains("webhook-token"));
    assert!(!outbound_debug.contains("telemetry-secret-value"));
    assert!(!outbound_debug.contains("webhook-secret-value"));
    let RuntimeGatewayStateStore::Postgres { tls, .. } = &config.state_store else {
        panic!("expected postgres state store");
    };
    assert_eq!(
        tls.mode(),
        prodex_storage_postgres_runtime::PostgresTlsMode::VerifyFull
    );
    assert_eq!(
        config.state_store.coordination_redis_url(),
        Some("rediss://redis.internal/0")
    );
    let initial_fingerprint = config.credential_fingerprint;
    let postgres_path = secrets
        .projected_root
        .as_ref()
        .unwrap()
        .join("postgres-url");
    std::fs::write(&postgres_path, "postgres://prodex@127.0.0.2/prodex").unwrap();
    let static_rotated = resolve_gateway_launch_config_with_secrets(
        &paths,
        &state,
        &gateway_args(),
        &policy,
        &secrets,
    )
    .unwrap();
    assert_eq!(static_rotated.credential_fingerprint, initial_fingerprint);
    let provider_path = secrets
        .projected_root
        .as_ref()
        .unwrap()
        .join("provider-key");
    std::fs::write(&provider_path, "rotated-provider-secret").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&provider_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let rotated = resolve_gateway_launch_config_with_secrets(
        &paths,
        &state,
        &gateway_args(),
        &policy,
        &secrets,
    )
    .unwrap();
    assert_eq!(rotated.credential_fingerprint, initial_fingerprint);

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
