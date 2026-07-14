use super::{
    AppState, Duration, RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, TestUpstream, app_paths_for_root,
    start_runtime_local_rewrite_proxy_with_harness, temp_root,
};

fn start_openai_harness_proxy(
    paths: &crate::AppPaths,
    upstream_base_url: String,
    harness: prodex_provider_core::HarnessMode,
) -> crate::RuntimeRotationProxy {
    start_runtime_local_rewrite_proxy_with_harness(
        RuntimeLocalRewriteProxyStartOptions {
            paths,
            state: &AppState::default(),
            upstream_base_url,
            provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
                api_keys: vec!["test-upstream-key".to_string()],
            },
            upstream_no_proxy: false,
            smart_context_enabled: false,
            presidio_redaction_enabled: false,
            model_context_window_tokens: None,
            preferred_listen_addr: Some("127.0.0.1:0"),
            gateway_auth_token_hash: None,
            gateway_admin_tokens: Vec::new(),
            gateway_sso: RuntimeGatewaySsoConfig::default(),
            gateway_state_store: RuntimeGatewayStateStore::file(paths),
            gateway_virtual_keys: Vec::new(),
            gateway_route_aliases: Vec::new(),
            gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
            gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
            gateway_call_id_header: None,
            gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        },
        prodex_provider_core::resolve_harness_mode(Some(harness), None),
    )
    .expect("harness proxy should start")
}

#[test]
fn native_harness_preserves_exact_request_bytes_through_local_bridge() {
    let root = temp_root("harness-native-exact");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let proxy = start_openai_harness_proxy(
        &paths,
        format!("http://{}/v1", upstream.addr),
        prodex_provider_core::HarnessMode::Native,
    );
    let body = br#"{  "model":"gpt-5.4", "stream":false, "input":"exact"  }"#;

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .header("content-type", "application/json")
        .body(body.to_vec())
        .send()
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
        body
    );
}

#[test]
fn minimal_harness_shapes_once_before_openai_provider_send() {
    let root = temp_root("harness-minimal-send");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let proxy = start_openai_harness_proxy(
        &paths,
        format!("http://{}/v1", upstream.addr),
        prodex_provider_core::HarnessMode::Minimal,
    );

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "implement",
            "previous_response_id": "resp_previous",
            "stream": false,
            "unknown_field": {"kept": true}
        }))
        .send()
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
    )
    .unwrap();
    let instructions = body["instructions"].as_str().unwrap();
    assert_eq!(
        instructions.matches("[Prodex harness: minimal/v1]").count(),
        1
    );
    assert_eq!(body["previous_response_id"], "resp_previous");
    assert_eq!(body["unknown_field"]["kept"], true);
}

#[test]
fn minimal_harness_rejects_structured_instructions_with_redacted_400() {
    let root = temp_root("harness-minimal-structured");
    let paths = app_paths_for_root(root);
    let proxy = start_openai_harness_proxy(
        &paths,
        "http://127.0.0.1:9/v1".to_string(),
        prodex_provider_core::HarnessMode::Minimal,
    );

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "implement",
            "instructions": {"secret-sentinel": "must-not-leak"}
        }))
        .send()
        .unwrap();

    assert_eq!(response.status().as_u16(), 400);
    let body = response.text().unwrap();
    assert!(body.contains("invalid_request"), "{body}");
    assert!(!body.contains("secret-sentinel"), "{body}");
    assert!(!body.contains("must-not-leak"), "{body}");
}

#[test]
fn minimal_harness_bypasses_compact_requests() {
    let root = temp_root("harness-minimal-compact");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let proxy = start_openai_harness_proxy(
        &paths,
        format!("http://{}/v1", upstream.addr),
        prodex_provider_core::HarnessMode::Minimal,
    );
    let body = br#"{ "model":"gpt-5.4", "instructions":{"structured":true}, "input":"keep" }"#;

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses/compact", proxy.listen_addr))
        .header("content-type", "application/json")
        .body(body.to_vec())
        .send()
        .unwrap();

    assert_eq!(response.status().as_u16(), 501);
}
