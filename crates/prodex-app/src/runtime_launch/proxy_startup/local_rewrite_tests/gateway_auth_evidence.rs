use super::*;

#[test]
fn gateway_oidc_missing_scope_claims_preserves_scim_role_and_tenant_fallback() {
    let root = temp_root("gateway-oidc-scim-fallback");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let jwks = TestJwksServer::start();
    let issuer = "https://idp.example";
    let audience = "prodex-gateway";
    let email = "oidc-scim@example.com";
    let tenant_id = "00000000-0000-7000-8000-000000000101";
    let admin_token = "scim-provisioner-token";
    let oidc_token = gateway_oidc_test_token_without_scope_claims(issuer, audience, email);
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: vec![runtime_gateway_test_admin_token(admin_token)],
        gateway_sso: RuntimeGatewaySsoConfig {
            proxy_token_hash: None,
            require_tenant: true,
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            oidc: Some(RuntimeGatewayOidcConfig {
                issuer: issuer.to_string(),
                audience: audience.to_string(),
                jwks_url: Some(format!("http://{}/jwks.json", jwks.addr)),
                jwks_origin_allowlist: Vec::new(),
                user_claim: "email".to_string(),
                role_claim: "prodex_role".to_string(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: "prodex_key_prefixes".to_string(),
                authentication_strength: None,
            }),
            browser: None,
            workload_identity: None,
        },
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");
    wait_for_oidc_cache(&proxy, 1);
    let client = reqwest::blocking::Client::new();

    let provisioned = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "userName": email,
            "active": true,
            "role": "admin",
            "tenant_id": tenant_id,
            "allowed_key_prefixes": ["scim-oidc-"]
        }))
        .send()
        .expect("SCIM fallback user should be provisioned");
    assert_eq!(provisioned.status().as_u16(), 201);

    let created = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(oidc_token)
        .json(&serde_json::json!({"name": "scim-oidc-key"}))
        .send()
        .expect("OIDC request should use SCIM role and tenant fallback");
    assert_eq!(created.status().as_u16(), 201);
    let created: serde_json::Value = created.json().expect("created key should be json");
    assert_eq!(created["key"]["tenant_id"], tenant_id);

    let unknown_role = gateway_oidc_test_token(issuer, audience, email, "owner", &[]);
    let rejected = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(unknown_role)
        .send()
        .expect("unknown OIDC role should be rejected");
    assert_eq!(rejected.status().as_u16(), 401);
    assert_eq!(
        rejected.json::<serde_json::Value>().unwrap()["error"]["code"],
        "invalid_admin_token"
    );
}
