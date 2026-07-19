use super::*;

#[test]
fn gateway_presidio_redaction_failure_is_audited_without_payload_or_endpoint_leakage() {
    let root = temp_root("gateway-presidio-redaction-failure-audit");
    let audit_dir = root.join("audit");
    std::fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = crate::TestEnvVarGuard::set(
        "PRODEX_AUDIT_LOG_DIR",
        audit_dir.to_str().expect("audit dir should be utf8"),
    );
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let gateway_token = "presidio-failure-gateway-token";
    let sensitive_input = "alice@example.com";
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
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
    })
    .expect("gateway proxy should start");
    crate::register_runtime_presidio_redaction_proxy_state(
        &proxy.log_path,
        Some(crate::RuntimePresidioRedactionConfig {
            analyzer_url: "http://127.0.0.1:9/analyze?secret=presidio-endpoint-secret".to_string(),
            anonymizer_url: "http://127.0.0.1:9/anonymize?secret=presidio-endpoint-secret"
                .to_string(),
            languages: vec!["en".to_string()],
            language_mode: crate::PresidioLanguageMode::Fixed,
            fail_closed: true,
            trusted_hosts: Vec::new(),
            timeout_ms: 10_000,
            max_response_bytes: 4 * 1024 * 1024,
            max_concurrency: 8,
        }),
    )
    .expect("Presidio state should register");

    let rejected = reqwest::blocking::Client::new()
        .post(format!(
            "http://{}/v1/responses?token=gateway-query-secret",
            proxy.listen_addr
        ))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({"model":"gpt-5","input":sensitive_input}))
        .send()
        .expect("presidio failure request should be sent");
    assert_eq!(rejected.status().as_u16(), 502);
    let body = rejected.text().expect("rejection should be text");
    assert_eq!(body, "gateway PII redaction failed");
    assert!(
        upstream
            .body_rx
            .recv_timeout(Duration::from_millis(200))
            .is_err()
    );

    let audit_log = std::fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("presidio failure should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"presidio_redaction_failed""#));
    assert!(audit_log.contains(r#""reason":"presidio_redaction_failed""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(gateway_token));
    assert!(!audit_log.contains(sensitive_input));
    assert!(!audit_log.contains("presidio-endpoint-secret"));
    assert!(!audit_log.contains("127.0.0.1:9"));

    let runtime_log =
        std::fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("local_rewrite_presidio_redaction_failed"));
    assert!(runtime_log.contains("reason=presidio_redaction_failed"));
    assert!(!runtime_log.contains(gateway_token));
    assert!(!runtime_log.contains(sensitive_input));
    assert!(!runtime_log.contains("presidio-endpoint-secret"));
    assert!(!runtime_log.contains("gateway-query-secret"));
    assert!(!runtime_log.contains("127.0.0.1:9"));
}

#[test]
fn gateway_presidio_fail_open_forwards_request_when_local_inspection_hits_limits() {
    let root = temp_root("gateway-presidio-local-limit-fail-open");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let gateway_token = "fail-open-gateway-token";
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
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
    })
    .expect("gateway proxy should start");
    crate::register_runtime_presidio_redaction_proxy_state(
        &proxy.log_path,
        Some(crate::RuntimePresidioRedactionConfig {
            analyzer_url: "http://127.0.0.1:9/analyze".to_string(),
            anonymizer_url: "http://127.0.0.1:9/anonymize".to_string(),
            languages: vec!["en".to_string()],
            language_mode: crate::PresidioLanguageMode::Fixed,
            fail_closed: false,
            trusted_hosts: Vec::new(),
            timeout_ms: 1_000,
            max_response_bytes: 1024,
            max_concurrency: 1,
        }),
    )
    .expect("Presidio state should register");

    let mut deep_input = serde_json::json!("safe input");
    for _ in 0..40 {
        deep_input = serde_json::json!({"input": deep_input});
    }
    let request_body = serde_json::to_vec(&serde_json::json!({
        "model": "gpt-5",
        "input": deep_input,
    }))
    .unwrap();
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .header("content-type", "application/json")
        .body(request_body.clone())
        .send()
        .expect("fail-open request should be sent");

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("fail-open request should reach upstream"),
        request_body
    );
    let runtime_log = std::fs::read_to_string(&proxy.log_path).unwrap();
    assert!(runtime_log.contains("presidio_redaction_error"));
    assert!(runtime_log.contains("fail_mode=open"));
    assert!(!runtime_log.contains("local_rewrite_presidio_redaction_failed"));
}
