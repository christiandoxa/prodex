use super::{
    AppState, RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, TestUpstream, app_paths_for_root,
    start_runtime_local_rewrite_proxy, temp_root,
};

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

    let crafted_data_plane_path = reqwest::blocking::Client::new()
        .get(format!(
            "http://{}/v1/models/example/prodex/gateway/admin",
            proxy.listen_addr
        ))
        .send()
        .expect("crafted data-plane admin path should be sent");
    assert_eq!(crafted_data_plane_path.status().as_u16(), 401);
}
