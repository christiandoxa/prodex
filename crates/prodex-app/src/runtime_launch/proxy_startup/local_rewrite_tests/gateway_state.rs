use super::support::*;
use crate::AppState;
use crate::runtime_launch::proxy_startup::local_rewrite::{
    RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeGatewayVirtualKeyUsageDelta,
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
    runtime_gateway_virtual_key_usage_apply_deltas, start_runtime_local_rewrite_proxy,
};
use std::fs;
use std::time::Duration;

#[test]
fn gateway_usage_delta_store_merges_batches_without_losing_counts() {
    let root = temp_root("gateway-usage-delta-merge");
    let path = root.join("gateway-virtual-key-usage.json");
    let ledger_path = root.join("gateway-billing-ledger.jsonl");
    let state_store = RuntimeGatewayStateStore::File {
        key_store_path: root.join("gateway-virtual-keys.json"),
        usage_path: path.clone(),
        ledger_path: ledger_path.clone(),
    };

    runtime_gateway_virtual_key_usage_apply_deltas(
        &state_store,
        &[RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 1,
            key_name: "team-a".to_string(),
            model: "gpt-5.4".to_string(),
            minute_epoch: 100,
            input_tokens: 7,
            estimated_cost_microusd: Some(11),
            created_at_epoch: 1_700_000_000,
        }],
    )
    .expect("first delta batch should save");
    runtime_gateway_virtual_key_usage_apply_deltas(
        &state_store,
        &[RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 2,
            key_name: "team-a".to_string(),
            model: "gpt-5.4".to_string(),
            minute_epoch: 100,
            input_tokens: 13,
            estimated_cost_microusd: Some(17),
            created_at_epoch: 1_700_000_001,
        }],
    )
    .expect("second delta batch should merge");

    let usage = wait_for_json_file(&path);
    assert_eq!(usage["team-a"]["requests_total"], 2);
    assert_eq!(usage["team-a"]["requests_this_minute"], 2);
    assert_eq!(usage["team-a"]["tokens_this_minute"], 20);
    assert_eq!(usage["team-a"]["spend_microusd"], 28);
    let ledger = fs::read_to_string(&ledger_path).expect("ledger should be written");
    assert_eq!(ledger.lines().count(), 2);
    assert!(ledger.contains("\"call_id\":\"prodex-1\""));
    assert!(ledger.contains("\"estimated_cost_microusd\":17"));
}

#[test]
fn gateway_sqlite_state_store_persists_admin_keys_and_usage() {
    let root = temp_root("gateway-sqlite-state");
    let paths = app_paths_for_root(root.clone());
    let db_path = root.join("gateway-state.sqlite");
    let state_store = RuntimeGatewayStateStore::sqlite(db_path.clone());
    let upstream = TestUpstream::start();
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
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            admin_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: state_store.clone(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("sqlite gateway proxy should start");
    let client = reqwest::blocking::Client::new();
    let created = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"name": "team-sqlite"}))
        .send()
        .expect("sqlite create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    let created: serde_json::Value = created.json().expect("create response should be json");
    let token = created["token"]
        .as_str()
        .expect("generated sqlite token should be returned")
        .to_string();

    let response = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "sqlite"}))
        .send()
        .expect("sqlite virtual key request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive sqlite request");
    wait_for_sqlite_usage_total(&db_path, "team-sqlite", 1);
    wait_for_sqlite_ledger_response_status(&db_path, 2, 200);
    drop(proxy);

    let restarted = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
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
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: state_store,
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("sqlite gateway proxy should restart");
    let keys = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            restarted.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("sqlite key list request should be sent");
    assert_eq!(keys.status().as_u16(), 200);
    let keys: serde_json::Value = keys.json().expect("key list response should be json");
    assert_eq!(keys["state_backend"], "sqlite");
    assert_eq!(keys["keys"][0]["name"], "team-sqlite");
    assert_eq!(keys["keys"][0]["usage"]["requests_total"], 1);
    let ledger = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger",
            restarted.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("sqlite ledger request should be sent");
    assert_eq!(ledger.status().as_u16(), 200);
    let ledger: serde_json::Value = ledger.json().expect("ledger response should be json");
    assert_eq!(ledger["state_backend"], "sqlite");
    assert_eq!(ledger["records"][0]["key_name"], "team-sqlite");
    assert_eq!(ledger["records"][0]["call_id"], "prodex-2");
    assert_eq!(ledger["records"][0]["response_status"], 200);
    assert_eq!(ledger["records"][0]["output_tokens"], 11);
    assert!(db_path.exists());
    assert!(!root.join("gateway-virtual-keys.json").exists());
}
