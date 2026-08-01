use std::thread;

use tiny_http::{Response as TinyResponse, Server as TinyServer};

use super::super::*;

fn start_guardrail_webhook_response(status_code: u16, body: &'static str) -> std::net::SocketAddr {
    let server = TinyServer::http("127.0.0.1:0").expect("guardrail webhook should bind");
    let addr = server
        .server_addr()
        .to_ip()
        .expect("guardrail webhook should expose TCP addr");
    thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let _ = request.respond(TinyResponse::from_string(body).with_status_code(status_code));
        }
    });
    addr
}

#[test]
fn gateway_guardrail_webhook_fail_closed_blocks_missing_allow_field() {
    let root = temp_root("gateway-guardrail-webhook-missing-allow");
    let paths = app_paths_for_root(root.clone());
    let webhook_addr = start_guardrail_webhook_response(200, "{}");
    let upstream = TestUpstream::start_n(0);
    let virtual_token = "team-a-token";
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
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig {
            url: Some(format!("http://{webhook_addr}/check")),
            phases: vec!["pre".to_string()],
            bearer_token: None,
            fail_closed: true,
        },
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(virtual_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "hello"}))
        .send()
        .expect("gateway request should be sent");
    assert_eq!(response.status().as_u16(), 403);
    let body: serde_json::Value = response.json().expect("error response should be json");
    assert_eq!(body["error"]["code"], "policy_violation");
    assert!(!root.join("gateway-virtual-key-usage.json").exists());
    assert!(!root.join("gateway-billing-ledger.jsonl").exists());
    let runtime_log = crate::read_runtime_proxy_test_log(&proxy.log_path);
    assert!(runtime_log.contains("gateway_guardrail_webhook_failed"));
    assert!(runtime_log.contains("error_kind=response_schema"));
}

#[test]
fn gateway_guardrail_webhook_fail_open_logs_invalid_responses_without_blocking() {
    for (case, status_code, webhook_body, error_kind) in [
        (
            "status",
            503,
            "do-not-log-non-success-webhook-body",
            "http_status",
        ),
        ("decode", 200, "do-not-log-malformed-webhook-body", "decode"),
    ] {
        let root = temp_root(&format!("gateway-guardrail-webhook-fail-open-{case}"));
        let paths = app_paths_for_root(root);
        let webhook_addr = start_guardrail_webhook_response(status_code, webhook_body);
        let upstream = TestUpstream::start();
        let gateway_token = "fail-open-gateway-token";
        let endpoint_secret = "do-not-log-webhook-query";
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
            gateway_auth_token_hash: Some(
                runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(gateway_token),
            ),
            gateway_admin_tokens: Vec::new(),
            gateway_sso: RuntimeGatewaySsoConfig::default(),
            gateway_state_store: RuntimeGatewayStateStore::file(&paths),
            gateway_virtual_keys: Vec::new(),
            gateway_route_aliases: Vec::new(),
            gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
            gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig {
                url: Some(format!(
                    "http://{webhook_addr}/check?secret={endpoint_secret}"
                )),
                phases: vec!["pre".to_string()],
                bearer_token: None,
                fail_closed: false,
            },
            gateway_call_id_header: Some("x-prodex-call-id".to_string()),
            gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        })
        .expect("gateway proxy should start");

        let response = reqwest::blocking::Client::new()
            .post(format!("http://{}/v1/responses", proxy.listen_addr))
            .bearer_auth(gateway_token)
            .json(&serde_json::json!({"model": "gpt-5.4", "input": "hello"}))
            .send()
            .expect("gateway request should be sent");
        assert_eq!(response.status().as_u16(), 200, "case={case}");

        let runtime_log = crate::read_runtime_proxy_test_log(&proxy.log_path);
        assert!(runtime_log.contains("gateway_guardrail_webhook_failed"));
        assert!(runtime_log.contains("endpoint=redacted"));
        assert!(runtime_log.contains(&format!("error_kind={error_kind}")));
        assert!(!runtime_log.contains(endpoint_secret));
        assert!(!runtime_log.contains(webhook_body));
        assert!(!runtime_log.contains(gateway_token));
    }
}
