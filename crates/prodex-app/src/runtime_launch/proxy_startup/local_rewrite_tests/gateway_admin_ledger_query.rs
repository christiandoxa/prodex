use super::{
    AppState, RuntimeGatewayAdminRole, RuntimeGatewayAdminToken,
    RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, app_paths_for_root, start_runtime_local_rewrite_proxy,
    temp_root,
};
use crate::runtime_launch::proxy_startup::local_rewrite_tests::support::TestUpstream;
use std::fs;

#[test]
fn gateway_admin_ledger_rejects_duplicate_limit_query_values() {
    let root = temp_root("gateway-admin-ledger-duplicate-limit");
    let paths = app_paths_for_root(root.clone());
    fs::create_dir_all(root.join("gateway-billing-ledger.jsonl"))
        .expect("ledger path directory should be created");
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-ledger-duplicate-limit-token";
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
            "http://{}/v1/prodex/gateway/ledger?limit=1&limit=2",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("ledger request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("response should be json");
    assert_eq!(body["error"]["code"], "pagination_limit_invalid");
    assert_eq!(body["error"]["message"], "pagination limit is invalid");
}

#[test]
fn gateway_admin_ledger_rejects_invalid_limit_query_value() {
    let root = temp_root("gateway-admin-ledger-invalid-limit");
    let paths = app_paths_for_root(root.clone());
    fs::create_dir_all(root.join("gateway-billing-ledger.jsonl"))
        .expect("ledger path directory should be created");
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-ledger-invalid-limit-token";
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
            "http://{}/v1/prodex/gateway/ledger?limit=abc",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("ledger request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("response should be json");
    assert_eq!(body["error"]["code"], "pagination_limit_invalid");
    assert_eq!(body["error"]["message"], "pagination limit is invalid");
}
