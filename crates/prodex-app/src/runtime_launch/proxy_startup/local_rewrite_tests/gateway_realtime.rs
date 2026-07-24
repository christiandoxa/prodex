use super::super::super::gemini_rewrite::RuntimeGeminiProviderAuth;
use super::super::{
    AppState, RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, app_paths_for_root, start_runtime_local_rewrite_proxy,
    temp_root, wait_for_ledger_file_key_response_status, wait_for_usage_file,
};

#[test]
fn gateway_realtime_websocket_requires_auth_and_reserves_bounded_virtual_key_usage() {
    let root = temp_root("gateway-realtime-websocket-auth");
    let paths = app_paths_for_root(root.clone());
    let virtual_token = "team-a-token";
    let upstream = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let upstream_addr = upstream.local_addr().unwrap();
    let upstream_thread = std::thread::spawn(move || {
        let _ = upstream.accept();
    });
    let _live_url = crate::TestEnvVarGuard::set(
        "PRODEX_GEMINI_LIVE_URL",
        &format!("ws://{upstream_addr}/live"),
    );
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
        provider: RuntimeLocalRewriteProviderOptions::Gemini {
            auth: RuntimeGeminiProviderAuth::ApiKeys {
                api_keys: vec!["gemini-key".to_string()],
            },
            thinking_budget_tokens: None,
            model_resolution: crate::RuntimeGeminiModelResolution::from_current_settings(),
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: vec![runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(virtual_token),
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
        }],
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let handshake = || {
        reqwest::blocking::Client::new()
            .get(format!("http://{}/v1/realtime", proxy.listen_addr))
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .header(
                "Sec-WebSocket-Key",
                ["dGhl", "IHNhbXBs", "ZSBub25jZQ=="].concat(),
            )
            .header("Sec-WebSocket-Version", "13")
    };
    let response = handshake()
        .send()
        .expect("websocket handshake request should be sent");
    assert_eq!(response.status().as_u16(), 401);
    let body: serde_json::Value = response.json().expect("error response should be json");
    assert_eq!(body["error"]["code"], "invalid_gateway_key");

    let response = handshake()
        .bearer_auth(virtual_token)
        .send()
        .expect("authorized websocket handshake request should be sent");
    assert_eq!(response.status().as_u16(), 502);
    upstream_thread.join().unwrap();

    let usage = wait_for_usage_file(&root.join("gateway-virtual-key-usage.json"));
    assert_eq!(usage["team-a"]["requests_total"], 1);
    assert_eq!(usage["team-a"]["tokens_this_minute"], 32_768);
    wait_for_ledger_file_key_response_status(
        &root.join("gateway-billing-ledger.jsonl"),
        "team-a",
        502,
    );
}
