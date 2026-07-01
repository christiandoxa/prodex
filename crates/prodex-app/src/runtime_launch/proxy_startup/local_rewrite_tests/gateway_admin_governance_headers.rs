use super::{
    AppState, RuntimeGatewayAdminRole, RuntimeGatewayAdminToken,
    RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, app_paths_for_root, start_runtime_local_rewrite_proxy,
    temp_root,
};
use crate::runtime_launch::proxy_startup::local_rewrite_tests::support::TestUpstream;

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
        .json(&serde_json::json!({"name": "team-etag-dup"}))
        .send()
        .expect("create key should be sent");
    assert_eq!(created.status().as_u16(), 201);

    let response = client
        .patch(&key_url)
        .bearer_auth(admin_token)
        .header("If-Match", "\"gateway-key-1\"")
        .header("If-Match", "\"gateway-key-2\"")
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("duplicate if-match request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let response: serde_json::Value = response.json().expect("response should be json");
    assert_eq!(response["error"]["code"], "entity_tag_invalid");
}
