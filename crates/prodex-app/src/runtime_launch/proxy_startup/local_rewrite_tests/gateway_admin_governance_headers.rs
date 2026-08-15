use super::{
    AppState, RuntimeGatewayAdminRole, RuntimeGatewayAdminToken,
    RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, app_paths_for_root, start_runtime_local_rewrite_proxy,
    temp_root,
};
use crate::runtime_launch::proxy_startup::local_rewrite_tests::support::TestUpstream;
use std::time::Duration;

#[test]
fn gateway_admin_mutation_rejects_duplicate_idempotency_key_headers() {
    let root = temp_root("gateway-admin-idempotency-duplicate-header");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-idempotency-duplicate-header-token";
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
            allowed_key_prefixes: Vec::new(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
        }],
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

    let client = reqwest::blocking::Client::new();
    let create_url = format!("http://{}/v1/prodex/gateway/keys", proxy.listen_addr);
    let response = client
        .post(&create_url)
        .bearer_auth(admin_token)
        .header("Idempotency-Key", "idem-create-key-1")
        .header("Idempotency-Key", "idem-create-key-2")
        .json(&serde_json::json!({"name": "team-idem-dup"}))
        .send()
        .expect("duplicate idempotency header request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let response: serde_json::Value = response.json().expect("response should be json");
    assert_eq!(response["error"]["code"], "idempotency_key_invalid");
}

#[test]
fn gateway_admin_mutation_rejects_empty_idempotency_key_with_shared_boundary_error() {
    let root = temp_root("gateway-admin-idempotency-empty-header");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-idempotency-empty-header-token";
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
            allowed_key_prefixes: Vec::new(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
        }],
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

    let client = reqwest::blocking::Client::new();
    let create_url = format!("http://{}/v1/prodex/gateway/keys", proxy.listen_addr);
    let response = client
        .post(&create_url)
        .bearer_auth(admin_token)
        .header("Idempotency-Key", "")
        .json(&serde_json::json!({"name": "team-idem-empty"}))
        .send()
        .expect("empty idempotency header request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let response: serde_json::Value = response.json().expect("response should be json");
    assert_eq!(response["error"]["code"], "idempotency_key_invalid");
}

#[test]
fn gateway_admin_key_mutations_reject_duplicate_if_match_headers() {
    let root = temp_root("gateway-admin-etag-duplicate-header");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-etag-duplicate-header-token";
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
            allowed_key_prefixes: Vec::new(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
        }],
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

    let client = reqwest::blocking::Client::new();
    let keys_url = format!("http://{}/v1/prodex/gateway/keys", proxy.listen_addr);
    let key_url = format!("{keys_url}/team-etag-dup");
    let created = client
        .post(&keys_url)
        .bearer_auth(admin_token)
        .header("Idempotency-Key", "duplicate-etag-create")
        .json(&serde_json::json!({"name": "team-etag-dup"}))
        .send()
        .expect("create key should be sent");
    assert_eq!(created.status().as_u16(), 201);

    let response = client
        .patch(&key_url)
        .bearer_auth(admin_token)
        .header("Idempotency-Key", "duplicate-etag-patch")
        .header("If-Match", "\"gateway-key-1\"")
        .header("If-Match", "\"gateway-key-2\"")
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("duplicate if-match request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let response: serde_json::Value = response.json().expect("response should be json");
    assert_eq!(response["error"]["code"], "entity_tag_invalid");
}

#[test]
fn gateway_openai_passthrough_filters_configured_sso_headers() {
    let root = temp_root("gateway-sso-header-filter");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let virtual_token = "team-a-token";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: Vec::new(),
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig {
            token_header: "x-auth-token".to_string(),
            user_header: "x-auth-user".to_string(),
            role_header: "x-auth-role".to_string(),
            tenant_header: "x-auth-tenant".to_string(),
            key_prefixes_header: "x-auth-prefixes".to_string(),
            ..RuntimeGatewaySsoConfig::default()
        },
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: vec![runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(virtual_token),
            allowed_models: vec!["gpt-5.4".to_string()],
            budget_microusd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
        }],
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(virtual_token)
        .header("ChatGPT-Account-Id", "acct-client")
        .header("X-AUTH-TOKEN", "fixture-sso-token")
        .header("x-auth-user", "fixture-user")
        .header("x-auth-role", "fixture-role")
        .header("x-auth-tenant", "fixture-tenant")
        .header("x-auth-prefixes", "team-a-")
        .header("x-codex-turn-state", "fixture-turn-state")
        .header("User-Agent", "fixture-user-agent")
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "hello"}))
        .send()
        .expect("gateway request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive gateway request");
    for name in [
        "authorization",
        "chatgpt-account-id",
        "x-auth-token",
        "x-auth-user",
        "x-auth-role",
        "x-auth-tenant",
        "x-auth-prefixes",
    ] {
        assert!(
            headers
                .iter()
                .all(|(header, _)| !header.eq_ignore_ascii_case(name)),
            "gateway credential header must not reach provider: {name} {headers:?}"
        );
    }
    assert!(headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("x-codex-turn-state") && value == "fixture-turn-state"
    }));
    assert!(headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("user-agent") && value == "fixture-user-agent"
    }));
}
