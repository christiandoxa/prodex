use std::time::Duration;

use super::{
    AppState, RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, TestUpstream, app_paths_for_root,
    start_runtime_local_rewrite_proxy, temp_root,
};

#[test]
fn gateway_application_boundary_preserves_legacy_auth_responses_and_side_effects() {
    let root = temp_root("gateway-application-boundary-parity");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(1);
    let gateway_token = "gateway-token";
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
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");
    let snapshot = proxy
        .gateway_side_effect_snapshot
        .as_ref()
        .expect("gateway side-effect snapshot should exist");
    let before = snapshot();
    let client = reqwest::blocking::Client::new();

    let rejected = client
        .post(format!(
            "http://{}/v1/responses?stream=false",
            proxy.listen_addr
        ))
        .bearer_auth("wrong-token")
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "hello"}))
        .send()
        .expect("rejected gateway request should be sent");
    assert_eq!(rejected.status().as_u16(), 401);
    assert_eq!(
        rejected.text().unwrap(),
        "missing or invalid gateway bearer token"
    );
    assert_eq!(
        snapshot(),
        before,
        "rejected auth must not mutate runtime/accounting state"
    );
    assert!(
        upstream
            .path_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "rejected auth must not reach the provider"
    );

    let accepted = client
        .post(format!(
            "http://{}/v1/responses?stream=false",
            proxy.listen_addr
        ))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "hello"}))
        .send()
        .expect("accepted gateway request should be sent");
    assert_eq!(accepted.status().as_u16(), 200);
    assert_eq!(
        upstream
            .path_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("provider should receive the accepted request"),
        "/v1/responses?stream=false"
    );
    let after = snapshot();
    assert_eq!(after.pending_usage_deltas, 0);
    assert_eq!(after.usage_request_ids, 0);
    assert_eq!(after.usage_durable_reservations, 0);

    let unsupported_control = client
        .get(format!("http://{}/admin/keys", proxy.listen_addr))
        .send()
        .expect("unsupported compatibility control request should be sent");
    assert_eq!(unsupported_control.status().as_u16(), 404);
    assert_eq!(
        unsupported_control.json::<serde_json::Value>().unwrap()["error"]["code"],
        "route_not_available"
    );
    assert!(
        upstream
            .path_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "typed control routes must not fall through to a provider"
    );
}
