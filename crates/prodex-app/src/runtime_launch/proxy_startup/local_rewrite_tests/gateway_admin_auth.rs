use super::*;
use crate::TestEnvVarGuard;
use std::fs;

#[test]
fn gateway_sso_headers_can_authenticate_scoped_admin() {
    let root = temp_root("gateway-sso-admin");
    let audit_dir = root.join("audit");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
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
        gateway_admin_tokens: Vec::new(),
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

    let rejected = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", "wrong-token")
        .header("x-prodex-sso-user", "alice@example.com")
        .send()
        .expect("bad SSO request should be sent");
    assert_eq!(rejected.status().as_u16(), 401);

    let created = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .header("x-prodex-sso-role", "admin")
        .header("x-prodex-sso-key-prefixes", "team-a-")
        .json(&serde_json::json!({"name": "team-a-sso"}))
        .send()
        .expect("SSO admin create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);

    let forbidden = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .header("x-prodex-sso-role", "admin")
        .header("x-prodex-sso-key-prefixes", "team-a-")
        .json(&serde_json::json!({"name": "team-b-sso"}))
        .send()
        .expect("SSO admin forbidden create key request should be sent");
    assert_eq!(forbidden.status().as_u16(), 403);

    let listed = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .header("x-prodex-sso-role", "viewer")
        .header("x-prodex-sso-key-prefixes", "team-a-")
        .send()
        .expect("SSO viewer list key request should be sent");
    assert_eq!(listed.status().as_u16(), 200);
    let listed: serde_json::Value = listed.json().expect("SSO list response should be json");
    assert_eq!(listed["keys"][0]["name"], "team-a-sso");

    let viewer_write_forbidden = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .header("x-prodex-sso-role", "viewer")
        .header("x-prodex-sso-key-prefixes", "team-a-")
        .json(&serde_json::json!({"name": "team-a-viewer-denied"}))
        .send()
        .expect("SSO viewer forbidden create key request should be sent");
    assert_eq!(viewer_write_forbidden.status().as_u16(), 403);
    assert_eq!(
        viewer_write_forbidden.json::<serde_json::Value>().unwrap()["error"]["code"],
        "gateway_admin_role_forbidden"
    );

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("gateway admin audit log should be written");
    assert!(audit_log.contains(r#""action":"authorization_denied""#));
    assert!(audit_log.contains(r#""reason":"role_forbidden""#));
    assert!(audit_log.contains(r#""role":"viewer""#));
    assert!(!audit_log.contains(sso_token));
}

#[test]
fn gateway_sso_missing_or_unknown_role_never_uses_admin_default() {
    let root = temp_root("gateway-sso-missing-role");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
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
        gateway_admin_tokens: Vec::new(),
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

    for role in [None, Some("not-admin")] {
        let mut request = client
            .post(format!(
                "http://{}/v1/prodex/gateway/keys",
                proxy.listen_addr
            ))
            .header("x-prodex-sso-token", sso_token)
            .header("x-prodex-sso-user", "alice@example.com")
            .json(&serde_json::json!({"name": "team-a-denied"}));
        if let Some(role) = role {
            request = request.header("x-prodex-sso-role", role);
        }
        let response = request
            .send()
            .expect("SSO missing/unknown role request should be sent");
        assert_eq!(response.status().as_u16(), 403);
        assert_eq!(
            response.json::<serde_json::Value>().unwrap()["error"]["code"],
            "gateway_admin_role_forbidden"
        );
    }
}

#[test]
fn gateway_sso_admin_auth_fails_closed_when_scim_store_becomes_invalid() {
    let root = temp_root("gateway-sso-invalid-scim-store");
    let paths = app_paths_for_root(root.clone());
    let upstream = TestUpstream::start_n(0);
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
        gateway_admin_tokens: Vec::new(),
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
        },
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start with missing store");
    fs::write(root.join("gateway-virtual-keys.json"), "{")
        .expect("invalid gateway key store should be written after startup");

    let rejected = reqwest::blocking::Client::new()
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .send()
        .expect("SSO request should be sent");
    assert_eq!(rejected.status().as_u16(), 401);
}

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
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            admin_token,
        )),
        gateway_admin_tokens: Vec::new(),
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
        .post(format!(
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
        .post(format!(
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
        .post(format!(
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
        .patch(format!(
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

#[test]
fn gateway_oidc_jwt_can_authenticate_scoped_admin() {
    let root = temp_root("gateway-oidc-admin");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let jwks = TestJwksServer::start();
    let issuer = "https://idp.example";
    let audience = "prodex-gateway";
    let token =
        gateway_oidc_test_token(issuer, audience, "alice@example.com", "admin", &["team-a-"]);
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
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig {
            proxy_token_hash: None,
            require_tenant: false,
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            oidc: Some(RuntimeGatewayOidcConfig {
                issuer: issuer.to_string(),
                audience: audience.to_string(),
                jwks_url: Some(format!("http://{}/jwks.json", jwks.addr)),
                user_claim: "email".to_string(),
                role_claim: "prodex_role".to_string(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: "prodex_key_prefixes".to_string(),
            }),
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

    let created = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-a-oidc"}))
        .send()
        .expect("OIDC admin create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);

    let forbidden = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-b-oidc"}))
        .send()
        .expect("OIDC admin forbidden key request should be sent");
    assert_eq!(forbidden.status().as_u16(), 403);

    let listed = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .send()
        .expect("OIDC admin list key request should be sent");
    assert_eq!(listed.status().as_u16(), 200);
    let listed: serde_json::Value = listed.json().expect("OIDC list response should be json");
    assert_eq!(listed["keys"][0]["name"], "team-a-oidc");
    assert_eq!(
        jwks.request_count(),
        1,
        "JWKS should be cached across OIDC admin requests"
    );
}

#[test]
fn gateway_oidc_requires_tenant_when_configured() {
    let root = temp_root("gateway-oidc-require-tenant");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let jwks = TestJwksServer::start();
    let issuer = "https://idp.example";
    let audience = "prodex-gateway";
    let token =
        gateway_oidc_test_token(issuer, audience, "alice@example.com", "admin", &["team-a-"]);
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
        gateway_admin_tokens: Vec::new(),
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
                user_claim: "email".to_string(),
                role_claim: "prodex_role".to_string(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: "prodex_key_prefixes".to_string(),
            }),
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

    let rejected = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(token)
        .send()
        .expect("OIDC missing tenant request should be sent");

    assert_eq!(rejected.status().as_u16(), 401);
}

#[test]
fn gateway_oidc_rejects_malformed_and_expired_tokens() {
    let root = temp_root("gateway-oidc-negative-tokens");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let jwks = TestJwksServer::start();
    let issuer = "https://idp.example";
    let audience = "prodex-gateway";
    let expired_token =
        gateway_oidc_test_token_with_exp(issuer, audience, "alice@example.com", "admin", &[], 1);
    let wrong_issuer_token = gateway_oidc_test_token(
        "https://wrong-idp.example",
        audience,
        "alice@example.com",
        "admin",
        &[],
    );
    let wrong_audience_token =
        gateway_oidc_test_token(issuer, "wrong-audience", "alice@example.com", "admin", &[]);
    let disallowed_algorithm_token = gateway_oidc_test_hs256_token(issuer, audience);
    let unknown_kid_token = gateway_oidc_test_token_with_exp_and_kid(
        issuer,
        audience,
        "alice@example.com",
        "admin",
        &[],
        u64::MAX,
        "unknown-key",
    );
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
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig {
            proxy_token_hash: None,
            require_tenant: false,
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            oidc: Some(RuntimeGatewayOidcConfig {
                issuer: issuer.to_string(),
                audience: audience.to_string(),
                jwks_url: Some(format!("http://{}/jwks.json", jwks.addr)),
                user_claim: "email".to_string(),
                role_claim: "prodex_role".to_string(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: "prodex_key_prefixes".to_string(),
            }),
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

    for token in [
        "not-a-jwt".to_string(),
        expired_token,
        wrong_issuer_token,
        wrong_audience_token,
        disallowed_algorithm_token,
        unknown_kid_token,
    ] {
        let rejected = client
            .get(format!(
                "http://{}/v1/prodex/gateway/keys",
                proxy.listen_addr
            ))
            .bearer_auth(token)
            .send()
            .expect("OIDC negative admin request should be sent");
        assert_eq!(rejected.status().as_u16(), 401);
    }
}

#[test]
fn gateway_oidc_jwt_can_discover_jwks_uri() {
    let root = temp_root("gateway-oidc-discovery-admin");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let oidc = TestOidcDiscoveryServer::start();
    let issuer = format!("http://{}", oidc.addr);
    let audience = "prodex-gateway";
    let token = gateway_oidc_test_token(
        &issuer,
        audience,
        "alice@example.com",
        "admin",
        &["team-a-"],
    );
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
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig {
            proxy_token_hash: None,
            require_tenant: false,
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            oidc: Some(RuntimeGatewayOidcConfig {
                issuer: issuer.clone(),
                audience: audience.to_string(),
                jwks_url: None,
                user_claim: "email".to_string(),
                role_claim: "prodex_role".to_string(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: "prodex_key_prefixes".to_string(),
            }),
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
    assert_eq!(
        oidc.request_count(),
        2,
        "OIDC discovery and JWKS should be prefetched before first admin request"
    );
    let client = reqwest::blocking::Client::new();

    let created = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-a-discovered-oidc"}))
        .send()
        .expect("OIDC discovery admin create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);
}

#[test]
fn authenticates_with_stale_while_revalidate_jwks_without_request_path_fetch() {
    let _ttl = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS", "0");
    let root = temp_root("gateway-oidc-stale-jwks");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let jwks = TestJwksServer::start_with_success_count(1);
    let issuer = "https://idp.example";
    let audience = "prodex-gateway";
    let token = gateway_oidc_test_token(
        issuer,
        audience,
        "alice@example.com",
        "admin",
        &["team-lkg-"],
    );
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
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig {
            proxy_token_hash: None,
            require_tenant: false,
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            oidc: Some(RuntimeGatewayOidcConfig {
                issuer: issuer.to_string(),
                audience: audience.to_string(),
                jwks_url: Some(format!("http://{}/jwks.json", jwks.addr)),
                user_claim: "email".to_string(),
                role_claim: "prodex_role".to_string(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: "prodex_key_prefixes".to_string(),
            }),
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

    let first = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-lkg-first"}))
        .send()
        .expect("first OIDC admin create key request should be sent");
    assert_eq!(first.status().as_u16(), 201);
    assert_eq!(
        jwks.request_count(),
        1,
        "startup prefetch should load JWKS before admin requests"
    );

    let second = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-lkg-second"}))
        .send()
        .expect("second OIDC admin create key request should be sent");
    assert_eq!(second.status().as_u16(), 201);
    assert_eq!(
        jwks.request_count(),
        1,
        "admin request path must not synchronously refresh stale JWKS"
    );
}

#[test]
fn gateway_oidc_missing_jwks_cache_does_not_fetch_on_request_path() {
    let root = temp_root("gateway-oidc-missing-jwks-cache");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let jwks = TestJwksServer::start_with_success_count(0);
    let issuer = "https://idp.example";
    let audience = "prodex-gateway";
    let token =
        gateway_oidc_test_token(issuer, audience, "alice@example.com", "admin", &["team-a-"]);
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
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig {
            proxy_token_hash: None,
            require_tenant: false,
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            oidc: Some(RuntimeGatewayOidcConfig {
                issuer: issuer.to_string(),
                audience: audience.to_string(),
                jwks_url: Some(format!("http://{}/jwks.json", jwks.addr)),
                user_claim: "email".to_string(),
                role_claim: "prodex_role".to_string(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: "prodex_key_prefixes".to_string(),
            }),
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
    assert_eq!(
        jwks.request_count(),
        1,
        "startup prefetch may attempt JWKS fetch once"
    );
    let client = reqwest::blocking::Client::new();

    let rejected = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(token)
        .send()
        .expect("OIDC request with missing JWKS cache should be sent");

    assert_eq!(rejected.status().as_u16(), 401);
    assert_eq!(
        jwks.request_count(),
        1,
        "request path must not fetch JWKS when startup prefetch failed"
    );
}
