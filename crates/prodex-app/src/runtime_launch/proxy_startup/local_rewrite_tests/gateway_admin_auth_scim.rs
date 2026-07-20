use super::{
    AppState, IdempotentClient, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewaySsoConfig, RuntimeGatewayStateStore,
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions, TestUpstream,
    app_paths_for_root, runtime_gateway_test_admin_token, start_runtime_local_rewrite_proxy,
    temp_root,
};

#[test]
fn gateway_scim_users_can_provision_sso_admin_scope() {
    let root = temp_root("gateway-scim-sso-admin");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-token";
    let sso_token = "sso-proxy-token";
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
            proxy_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                sso_token,
            )),
            require_tenant: false,
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            oidc: None,
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
    let client = reqwest::blocking::Client::new();

    let created_user = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "userName": "alice@example.com",
            "displayName": "Alice Example",
            "active": true,
            "urn:prodex:params:scim:schemas:gateway:2.0:User": {
                "role": "admin",
                "team_id": "team-a",
                "group_ids": ["engineering", "platform"],
                "department_id": "research",
                "allowed_key_prefixes": ["team-a-"]
            }
        }))
        .send()
        .expect("SCIM user create request should be sent");
    assert_eq!(created_user.status().as_u16(), 201);
    let created_user: serde_json::Value = created_user
        .json()
        .expect("SCIM create response should be json");
    let user_id = created_user["id"]
        .as_str()
        .expect("SCIM user id should be present")
        .to_string();
    assert_eq!(created_user["userName"], "alice@example.com");
    assert_eq!(
        created_user["group_ids"],
        serde_json::json!(["engineering", "platform"])
    );
    assert_eq!(created_user["department_id"], "research");
    assert_eq!(
        created_user["urn:prodex:params:scim:schemas:gateway:2.0:User"]["group_ids"],
        serde_json::json!(["engineering", "platform"])
    );

    let listed_users = client
        .get(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("SCIM list request should be sent");
    assert_eq!(listed_users.status().as_u16(), 200);
    let listed_users: serde_json::Value = listed_users
        .json()
        .expect("SCIM list response should be json");
    assert_eq!(listed_users["totalResults"], 1);

    let created_key = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .json(&serde_json::json!({"name": "team-a-scim"}))
        .send()
        .expect("SCIM-backed SSO create key request should be sent");
    assert_eq!(created_key.status().as_u16(), 201);
    let created_key: serde_json::Value = created_key
        .json()
        .expect("SCIM-backed SSO create key response should be json");
    assert_eq!(created_key["key"]["team_id"], "team-a");

    let forbidden_key = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .json(&serde_json::json!({"name": "team-b-scim"}))
        .send()
        .expect("SCIM-backed SSO forbidden key request should be sent");
    assert_eq!(forbidden_key.status().as_u16(), 403);

    let deactivated = client
        .idempotent_patch(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users/{}",
            proxy.listen_addr, user_id
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "Operations": [
                {"op": "replace", "path": "active", "value": false}
            ]
        }))
        .send()
        .expect("SCIM deactivate request should be sent");
    assert_eq!(deactivated.status().as_u16(), 200);
    let deactivated: serde_json::Value = deactivated
        .json()
        .expect("SCIM deactivate response should be json");
    assert_eq!(deactivated["active"], false);

    let inactive_rejected = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .send()
        .expect("inactive SCIM SSO request should be sent");
    assert_eq!(inactive_rejected.status().as_u16(), 401);
}
