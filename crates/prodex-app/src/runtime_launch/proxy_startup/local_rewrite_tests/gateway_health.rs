use super::{
    AppState, RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, TestUpstream, app_paths_for_root,
    start_runtime_local_rewrite_proxy, temp_root,
};

#[test]
fn gateway_operational_health_endpoints_are_public_and_machine_readable() {
    let root = temp_root("gateway-operational-health");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let gateway_token = "gateway-token";
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

    let client = reqwest::blocking::Client::new();
    for (path, probe) in [
        ("/livez", "livez"),
        ("/readyz", "readyz"),
        ("/startupz", "startupz"),
    ] {
        let response = client
            .get(format!("http://{}{}", proxy.listen_addr, path))
            .send()
            .expect("health request should be sent");
        assert_eq!(response.status().as_u16(), 200);
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(content_type.contains("application/json"));
        let body: serde_json::Value = response.json().expect("health response should be json");
        assert_eq!(body["object"], "gateway.health");
        assert_eq!(body["probe"], probe);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["ready"], true);
        assert_eq!(body["local_overload"], false);
        assert!(body["active_request_limit"].as_u64().is_some());

        let head = client
            .head(format!("http://{}{}", proxy.listen_addr, path))
            .send()
            .expect("health HEAD request should be sent");
        assert_eq!(head.status().as_u16(), 200);
        assert!(
            head.bytes()
                .expect("health HEAD response body should be readable")
                .is_empty()
        );

        let rejected_method = client
            .post(format!("http://{}{}", proxy.listen_addr, path))
            .send()
            .expect("health method rejection should be sent");
        assert_eq!(rejected_method.status().as_u16(), 405);
        assert_eq!(
            rejected_method
                .headers()
                .get("allow")
                .and_then(|value| value.to_str().ok()),
            Some("GET, HEAD")
        );
        let rejected_method: serde_json::Value = rejected_method
            .json()
            .expect("health method rejection should be json");
        assert_eq!(rejected_method["object"], "gateway.health");
        assert_eq!(rejected_method["probe"], probe);
        assert_eq!(rejected_method["status"], "method_not_allowed");
    }
}

#[test]
fn gateway_operational_health_exposes_active_policy_version() {
    prodex_runtime_policy::clear_runtime_policy_cache();
    let root = temp_root("gateway-health-policy-version");
    std::fs::write(root.join("policy.toml"), "version = 1\n").expect("policy should be written");
    let _home = crate::TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
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

    let body: serde_json::Value = reqwest::blocking::Client::new()
        .get(format!("http://{}/readyz", proxy.listen_addr))
        .send()
        .expect("health request should be sent")
        .json()
        .expect("health response should be json");
    assert_eq!(body["policy_version"], 1);
    prodex_runtime_policy::clear_runtime_policy_cache();
}
