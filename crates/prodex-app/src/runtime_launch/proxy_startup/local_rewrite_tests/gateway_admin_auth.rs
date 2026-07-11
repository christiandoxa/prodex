use super::*;
#[path = "gateway_auth_evidence.rs"]
mod evidence;
use crate::TestEnvVarGuard;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_migrate_compatibility_state,
    runtime_gateway_sqlite_create_current_schema_for_tests,
};
use postgres::NoTls;
use prodex_storage_postgres::{PostgresRuntimeMode, plan_postgres_migrations};
use std::fs;

fn runtime_gateway_postgres_create_current_schema_for_tests(url: &str) {
    let mut client = postgres::Client::connect(url, NoTls).expect("postgres should connect");
    let has_enterprise_schema: bool = client
        .query_one(
            "SELECT EXISTS(
                SELECT 1
                FROM information_schema.tables
                WHERE table_schema = current_schema()
                  AND table_name = 'prodex_tenants'
            )",
            &[],
        )
        .expect("postgres enterprise schema probe should load")
        .get(0);
    if !has_enterprise_schema {
        let plan = plan_postgres_migrations(PostgresRuntimeMode::ExternalMigrator)
            .expect("postgres schema plan should build");
        for migration in &plan.migrations {
            client
                .batch_execute(migration.sql)
                .expect("postgres enterprise migration should apply");
        }
    }
    let tls = prodex_storage_postgres_runtime::PostgresTlsConfig::explicit_disable();
    runtime_gateway_postgres_migrate_compatibility_state(url, &tls)
        .expect("postgres compatibility migrations should apply");
}

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
        .idempotent_post(format!(
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
        .idempotent_post(format!(
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
        .idempotent_post(format!(
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
            .idempotent_post(format!(
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
fn gateway_data_plane_token_cannot_access_admin_endpoint() {
    let root = temp_root("gateway-root-token-not-admin");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let gateway_token = "gateway-root-token";
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
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let response = reqwest::blocking::Client::new()
        .get(format!(
            "http://{}/v1/prodex/gateway/openapi.json",
            proxy.listen_addr
        ))
        .bearer_auth(gateway_token)
        .send()
        .expect("gateway root-token admin request should be sent");

    assert_eq!(response.status().as_u16(), 403);
    assert_eq!(
        response.json::<serde_json::Value>().unwrap()["error"]["code"],
        "admin_auth_not_configured"
    );
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
                jwks_origin_allowlist: Vec::new(),
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
    wait_for_oidc_cache(&proxy, 1);
    let _request_path_ttl = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS", "0");
    let _request_path_lkg =
        TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS", "0");
    let client = reqwest::blocking::Client::new();

    let created = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-a-oidc"}))
        .send()
        .expect("OIDC admin create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);

    let forbidden = client
        .idempotent_post(format!(
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
                jwks_origin_allowlist: Vec::new(),
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
    wait_for_oidc_cache(&proxy, 1);
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
                jwks_origin_allowlist: Vec::new(),
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
    wait_for_oidc_cache(&proxy, 1);
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
                jwks_origin_allowlist: Vec::new(),
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
    wait_for_oidc_cache(&proxy, 2);
    let client = reqwest::blocking::Client::new();

    let created = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-a-discovered-oidc"}))
        .send()
        .expect("OIDC discovery admin create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    assert_eq!(
        oidc.request_count(),
        2,
        "background OIDC prefetch should finish discovery and JWKS before the first admin request completes"
    );
}

#[test]
fn authenticates_with_stale_while_revalidate_jwks_without_request_path_fetch() {
    let _ttl = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS", "1");
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
                jwks_origin_allowlist: Vec::new(),
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
    wait_for_oidc_cache(&proxy, 1);
    let client = reqwest::blocking::Client::new();

    let first = client
        .idempotent_post(format!(
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

    let refresh_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while jwks.request_count() < 2 && std::time::Instant::now() < refresh_deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        jwks.request_count(),
        2,
        "background refresh should revalidate stale JWKS outside the request path"
    );

    let second = client
        .idempotent_post(format!(
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
        2,
        "admin request path must reuse cached JWKS while background refresh handles staleness"
    );
}

#[test]
fn expired_oidc_jwks_cache_fails_closed_without_request_path_fetch() {
    let _ttl = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS", "1");
    let _lkg = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS", "0");
    let _backoff = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS", "5000");
    let root = temp_root("gateway-oidc-expired-lkg-jwks");
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
        &["team-expired-lkg-"],
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
                jwks_origin_allowlist: Vec::new(),
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
    wait_for_oidc_cache(&proxy, 1);
    let client = reqwest::blocking::Client::new();

    let first = client
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-expired-lkg-first"}))
        .send()
        .expect("first OIDC admin create key request should be sent");
    assert_eq!(first.status().as_u16(), 201);
    assert_eq!(jwks.request_count(), 1);

    let refresh_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while jwks.request_count() < 2 && std::time::Instant::now() < refresh_deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(jwks.request_count(), 2);
    let request_count_before_rejected = jwks.request_count();

    let rejected = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .send()
        .expect("expired-LKG OIDC request should be sent");
    assert_eq!(rejected.status().as_u16(), 401);
    assert_eq!(
        jwks.request_count(),
        request_count_before_rejected,
        "request path must not fetch JWKS after the LKG window expires"
    );
}

#[test]
fn oidc_background_refresh_uses_jwks_cache_control_max_age() {
    let _ttl = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS", "30");
    let root = temp_root("gateway-oidc-cache-control-jwks");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let jwks = TestJwksServer::start_with_success_count_and_cache_control(2, Some("max-age=1"));
    let issuer = "https://idp.example";
    let audience = "prodex-gateway";
    let token = gateway_oidc_test_token(
        issuer,
        audience,
        "alice@example.com",
        "admin",
        &["team-cache-control-"],
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
                jwks_origin_allowlist: Vec::new(),
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
    wait_for_oidc_cache(&proxy, 1);

    let first = reqwest::blocking::Client::new()
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-cache-control-first"}))
        .send()
        .expect("first OIDC admin create key request should be sent");
    assert_eq!(first.status().as_u16(), 201);
    assert_eq!(jwks.request_count(), 1);

    let refresh_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while jwks.request_count() < 2 && std::time::Instant::now() < refresh_deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        jwks.request_count(),
        2,
        "Cache-Control max-age should override the longer fallback TTL"
    );

    let runtime_log = fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("gateway_oidc_refresh_metric"));
    assert!(runtime_log.contains("metric_name=prodex_oidc_refresh_events_total"));
    assert!(runtime_log.contains("oidc_refresh_operation=fetch_jwks"));
    assert!(runtime_log.contains("oidc_refresh_result=success"));
    assert!(runtime_log.contains("gateway_jwks_refresh_metric"));
    assert!(runtime_log.contains("metric_name=prodex_jwks_refresh_total"));
    assert!(runtime_log.contains("jwks_refresh_result=success"));
    assert!(runtime_log.contains("gateway_jwks_cache_age_metric"));
    assert!(runtime_log.contains("metric_name=prodex_jwks_cache_age_ms"));
    assert!(!runtime_log.contains("jwks.json"));
}

#[test]
fn oidc_background_refresh_retries_failed_jwks_after_backoff() {
    let _backoff = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS", "50");
    let root = temp_root("gateway-oidc-refresh-backoff");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let jwks = TestJwksServer::start_with_success_count(0);
    let issuer = "https://idp.example";
    let audience = "prodex-gateway";
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
                jwks_origin_allowlist: Vec::new(),
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

    let retry_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while jwks.request_count() < 2 && std::time::Instant::now() < retry_deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        jwks.request_count() >= 2,
        "failed background JWKS refresh should retry after the bounded backoff"
    );

    let runtime_log = fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("gateway_oidc_prefetch_failed"));
    assert!(runtime_log.contains("oidc_refresh_result=failed"));
    assert!(runtime_log.contains("oidc_refresh_result=backoff"));
    assert!(!runtime_log.contains("jwks.json"));
}

#[test]
fn gateway_oidc_missing_jwks_cache_does_not_fetch_on_request_path() {
    let _backoff = TestEnvVarGuard::set("PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS", "5000");
    let root = temp_root("gateway-oidc-missing-jwks-cache");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let jwks = TestJwksServer::start_with_delay(std::time::Duration::from_millis(500));
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
                jwks_origin_allowlist: Vec::new(),
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

    let fetch_deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while jwks.request_count() == 0 && std::time::Instant::now() < fetch_deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(jwks.request_count(), 1);
    let request_started = std::time::Instant::now();
    let rejected = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(token)
        .send()
        .expect("OIDC request with missing JWKS cache should be sent");

    assert_eq!(rejected.status().as_u16(), 401);
    assert!(
        request_started.elapsed() < std::time::Duration::from_millis(250),
        "request path must not spin-wait for background OIDC refresh"
    );
    assert_eq!(
        jwks.request_count(),
        1,
        "request path must not fetch JWKS when startup prefetch failed"
    );

    let log_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let runtime_log = loop {
        let runtime_log =
            fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
        if runtime_log.contains("oidc_refresh_result=failed")
            || std::time::Instant::now() >= log_deadline
        {
            break runtime_log;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert!(runtime_log.contains("gateway_oidc_refresh_metric"));
    assert!(runtime_log.contains("metric_name=prodex_oidc_refresh_events_total"));
    assert!(runtime_log.contains("oidc_refresh_operation=fetch_jwks"));
    assert!(runtime_log.contains("oidc_refresh_result=failed"));
    assert!(runtime_log.contains("gateway_jwks_refresh_metric"));
    assert!(runtime_log.contains("metric_name=prodex_jwks_refresh_total"));
    assert!(runtime_log.contains("jwks_refresh_result=failure"));
    assert!(!runtime_log.contains("jwks.json"));
}

#[test]
fn gateway_scim_create_keeps_compat_shape_on_sqlite_backend() {
    let root = temp_root("gateway-scim-sqlite-compat");
    let paths = app_paths_for_root(root.clone());
    let db_path = root.join("gateway-state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&db_path)
        .expect("sqlite schema fixture should be created");
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::sqlite(db_path),
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
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"userName": "sqlite@example.com", "tenant_id": prodex_domain::TenantId::new().to_string(), "user_id": prodex_domain::PrincipalId::new().to_string()}))
        .send()
        .expect("sqlite SCIM create request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    let created: serde_json::Value = created.json().expect("sqlite SCIM create json");
    assert_eq!(created["userName"], "sqlite@example.com");
    assert_eq!(created["externalId"], serde_json::Value::Null);
    assert_eq!(created["displayName"], serde_json::Value::Null);
}

#[test]
fn gateway_scim_update_keeps_compat_shape_on_sqlite_backend() {
    let root = temp_root("gateway-scim-sqlite-update-compat");
    let paths = app_paths_for_root(root.clone());
    let db_path = root.join("gateway-state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&db_path)
        .expect("sqlite schema fixture should be created");
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::sqlite(db_path),
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
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"userName": "sqlite-update@example.com", "tenant_id": prodex_domain::TenantId::new().to_string(), "user_id": prodex_domain::PrincipalId::new().to_string()}))
        .send()
        .expect("sqlite SCIM create request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    let created: serde_json::Value = created.json().expect("sqlite SCIM create json");
    let user_id = created["id"]
        .as_str()
        .expect("sqlite SCIM user id should be present")
        .to_string();

    let updated = client
        .idempotent_patch(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users/{}",
            proxy.listen_addr, user_id
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"displayName": "SQLite Update"}))
        .send()
        .expect("sqlite SCIM update request should be sent");
    assert_eq!(updated.status().as_u16(), 200);
    let updated: serde_json::Value = updated.json().expect("sqlite SCIM update json");
    assert_eq!(updated["userName"], "sqlite-update@example.com");
    assert_eq!(updated["displayName"], "SQLite Update");
    assert_eq!(updated["externalId"], serde_json::Value::Null);
}

#[test]
fn gateway_scim_delete_keeps_compat_shape_on_sqlite_backend() {
    let root = temp_root("gateway-scim-sqlite-delete-compat");
    let paths = app_paths_for_root(root.clone());
    let db_path = root.join("gateway-state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&db_path)
        .expect("sqlite schema fixture should be created");
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::sqlite(db_path),
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
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"userName": "sqlite-delete@example.com", "tenant_id": prodex_domain::TenantId::new().to_string(), "user_id": prodex_domain::PrincipalId::new().to_string()}))
        .send()
        .expect("sqlite SCIM create request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    let created: serde_json::Value = created.json().expect("sqlite SCIM create json");
    let user_id = created["id"]
        .as_str()
        .expect("sqlite SCIM user id should be present")
        .to_string();

    let deleted = client
        .idempotent_delete(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users/{}",
            proxy.listen_addr, user_id
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("sqlite SCIM delete request should be sent");
    assert_eq!(deleted.status().as_u16(), 200);
    let deleted: serde_json::Value = deleted.json().expect("sqlite SCIM delete json");
    assert_eq!(deleted["object"], "gateway.scim_user.deleted");
    assert_eq!(deleted["id"], user_id);
    assert_eq!(deleted["deleted"], true);
}

#[test]
fn gateway_scim_create_keeps_compat_shape_on_postgres_backend() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping: PRODEX_TEST_POSTGRES_URL is not set");
        return;
    };

    let root = temp_root("gateway-scim-postgres-compat");
    let paths = app_paths_for_root(root);
    runtime_gateway_postgres_create_current_schema_for_tests(&url);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::postgres(
            "PRODEX_TEST_POSTGRES_URL".to_string(),
            url,
        ),
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
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"userName": "postgres@example.com", "tenant_id": prodex_domain::TenantId::new().to_string(), "user_id": prodex_domain::PrincipalId::new().to_string()}))
        .send()
        .expect("postgres SCIM create request should be sent");
    let created_status = created.status().as_u16();
    let created_body = created
        .text()
        .expect("postgres SCIM create response should be readable");
    assert_eq!(
        created_status, 201,
        "postgres SCIM create failed: {created_body}"
    );
    let created: serde_json::Value =
        serde_json::from_str(&created_body).expect("postgres SCIM create json");
    assert_eq!(created["userName"], "postgres@example.com");
    assert_eq!(created["externalId"], serde_json::Value::Null);
    assert_eq!(created["displayName"], serde_json::Value::Null);
}

#[test]
fn gateway_scim_update_keeps_compat_shape_on_postgres_backend() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping: PRODEX_TEST_POSTGRES_URL is not set");
        return;
    };

    let root = temp_root("gateway-scim-postgres-update-compat");
    let paths = app_paths_for_root(root);
    runtime_gateway_postgres_create_current_schema_for_tests(&url);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::postgres(
            "PRODEX_TEST_POSTGRES_URL".to_string(),
            url,
        ),
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
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"userName": "postgres-update@example.com", "tenant_id": prodex_domain::TenantId::new().to_string(), "user_id": prodex_domain::PrincipalId::new().to_string()}))
        .send()
        .expect("postgres SCIM create request should be sent");
    let created_status = created.status().as_u16();
    let created_body = created
        .text()
        .expect("postgres SCIM create response should be readable");
    assert_eq!(
        created_status, 201,
        "postgres SCIM create failed: {created_body}"
    );
    let created: serde_json::Value =
        serde_json::from_str(&created_body).expect("postgres SCIM create json");
    let user_id = created["id"]
        .as_str()
        .expect("postgres SCIM user id should be present")
        .to_string();

    let updated = client
        .idempotent_patch(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users/{}",
            proxy.listen_addr, user_id
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"displayName": "Postgres Update"}))
        .send()
        .expect("postgres SCIM update request should be sent");
    let updated_status = updated.status().as_u16();
    let updated_body = updated
        .text()
        .expect("postgres SCIM update response should be readable");
    assert_eq!(
        updated_status, 200,
        "postgres SCIM update failed: {updated_body}"
    );
    let updated: serde_json::Value =
        serde_json::from_str(&updated_body).expect("postgres SCIM update json");
    assert_eq!(updated["userName"], "postgres-update@example.com");
    assert_eq!(updated["displayName"], "Postgres Update");
    assert_eq!(updated["externalId"], serde_json::Value::Null);
}

#[test]
fn gateway_scim_delete_keeps_compat_shape_on_postgres_backend() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping: PRODEX_TEST_POSTGRES_URL is not set");
        return;
    };

    let root = temp_root("gateway-scim-postgres-delete-compat");
    let paths = app_paths_for_root(root);
    runtime_gateway_postgres_create_current_schema_for_tests(&url);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-token";
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
        gateway_admin_tokens: vec![RuntimeGatewayAdminToken {
            name: "admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(admin_token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::postgres(
            "PRODEX_TEST_POSTGRES_URL".to_string(),
            url,
        ),
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
        .idempotent_post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"userName": "postgres-delete@example.com", "tenant_id": prodex_domain::TenantId::new().to_string(), "user_id": prodex_domain::PrincipalId::new().to_string()}))
        .send()
        .expect("postgres SCIM create request should be sent");
    let created_status = created.status().as_u16();
    let created_body = created
        .text()
        .expect("postgres SCIM create response should be readable");
    assert_eq!(
        created_status, 201,
        "postgres SCIM create failed: {created_body}"
    );
    let created: serde_json::Value =
        serde_json::from_str(&created_body).expect("postgres SCIM create json");
    let user_id = created["id"]
        .as_str()
        .expect("postgres SCIM user id should be present")
        .to_string();

    let deleted = client
        .idempotent_delete(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users/{}",
            proxy.listen_addr, user_id
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("postgres SCIM delete request should be sent");
    let deleted_status = deleted.status().as_u16();
    let deleted_body = deleted
        .text()
        .expect("postgres SCIM delete response should be readable");
    assert_eq!(
        deleted_status, 200,
        "postgres SCIM delete failed: {deleted_body}"
    );
    let deleted: serde_json::Value =
        serde_json::from_str(&deleted_body).expect("postgres SCIM delete json");
    assert_eq!(deleted["object"], "gateway.scim_user.deleted");
    assert_eq!(deleted["id"], user_id);
    assert_eq!(deleted["deleted"], true);
}
