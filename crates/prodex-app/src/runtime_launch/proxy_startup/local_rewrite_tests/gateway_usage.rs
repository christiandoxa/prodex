use super::*;

#[test]
fn gateway_virtual_key_usage_is_persisted_and_visible_to_admin_endpoint() {
    let root = temp_root("gateway-virtual-key-usage");
    let paths = app_paths_for_root(root.clone());
    let upstream = TestUpstream::start();
    let virtual_token = "team-a-token";
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
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: vec![runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "team-a".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: Some("team-a".to_string()),
            project_id: Some("project-a".to_string()),
            user_id: Some("alice@example.com".to_string()),
            budget_id: Some("budget-a".to_string()),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(virtual_token),
            allowed_models: vec!["gpt-5.4".to_string()],
            budget_microusd: Some(1_000_000),
            request_budget: Some(5),
            rpm_limit: Some(5),
            tpm_limit: Some(1_000),
        }],
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(virtual_token)
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "hello from a virtual key"
        }))
        .send()
        .expect("gateway request should be sent");

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response
            .headers()
            .get("x-prodex-call-id")
            .and_then(|value| value.to_str().ok()),
        Some("prodex-1")
    );
    let upstream_body = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive gateway request");
    assert!(String::from_utf8_lossy(&upstream_body.body).contains("gpt-5.4"));

    let usage = client
        .get(format!(
            "http://{}/v1/prodex/gateway/usage",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("admin usage request should be sent");
    assert_eq!(usage.status().as_u16(), 200);
    let usage: serde_json::Value = usage.json().expect("usage response should be json");
    assert_eq!(usage["object"], "gateway.usage");
    assert_eq!(usage["keys"][0]["name"], "team-a");
    assert_eq!(usage["keys"][0]["usage"]["requests_total"], 1);

    let metrics = client
        .get(format!(
            "http://{}/v1/prodex/gateway/metrics",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("admin metrics request should be sent");
    assert_eq!(metrics.status().as_u16(), 200);
    let metrics = metrics.text().expect("metrics response should be text");
    assert!(metrics.contains("prodex_gateway_virtual_key_requests_total"));
    assert!(metrics.contains("key=\"team-a\""));
    assert!(metrics.contains("source=\"policy\""));
    assert!(metrics.contains("tenant_id=\"tenant-a\""));
    assert!(metrics.contains("team_id=\"team-a\""));
    assert!(metrics.contains("project_id=\"project-a\""));
    assert!(metrics.contains("user_id=\"alice@example.com\""));
    assert!(metrics.contains("budget_id=\"budget-a\""));

    let rejected = client
        .get(format!(
            "http://{}/v1/prodex/gateway/usage",
            proxy.listen_addr
        ))
        .bearer_auth(virtual_token)
        .send()
        .expect("non-admin usage request should be sent");
    assert_eq!(rejected.status().as_u16(), 401);

    let persisted = wait_for_usage_file(&root.join("gateway-virtual-key-usage.json"));
    assert_eq!(persisted["team-a"]["requests_total"], 1);
    wait_for_ledger_file_response_status(&root.join("gateway-billing-ledger.jsonl"), 1, 200);
    let ledger = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("admin ledger request should be sent");
    assert_eq!(ledger.status().as_u16(), 200);
    let ledger: serde_json::Value = ledger.json().expect("ledger response should be json");
    assert_eq!(ledger["object"], "gateway.billing_ledger");
    assert_eq!(ledger["records"][0]["key_name"], "team-a");
    assert_eq!(ledger["records"][0]["call_id"], "prodex-1");
    assert_eq!(ledger["records"][0]["model"], "gpt-5.4");
    assert_eq!(ledger["records"][0]["response_status"], 200);
    assert_eq!(ledger["records"][0]["output_tokens"], 11);
    let ledger_csv = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger.csv",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("admin ledger CSV request should be sent");
    assert_eq!(ledger_csv.status().as_u16(), 200);
    assert!(
        ledger_csv
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .contains("text/csv")
    );
    let ledger_csv = ledger_csv.text().expect("ledger CSV should be text");
    assert!(ledger_csv.contains("call_id,key_name,model"));
    assert!(ledger_csv.contains("prodex-1,team-a,gpt-5.4"));
    let summary = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger/summary",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("admin ledger summary request should be sent");
    assert_eq!(summary.status().as_u16(), 200);
    let summary: serde_json::Value = summary.json().expect("summary response should be json");
    assert_eq!(summary["object"], "gateway.billing_summary");
    assert_eq!(summary["totals"]["requests"], 1);
    assert_eq!(summary["totals"]["successful_requests"], 1);
    assert_eq!(summary["totals"]["output_tokens"], 11);
    assert_eq!(summary["by_key"][0]["key_name"], "team-a");
    assert_eq!(summary["by_model"][0]["model"], "gpt-5.4");
    let summary_csv = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger/summary.csv",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("admin ledger summary CSV request should be sent");
    assert_eq!(summary_csv.status().as_u16(), 200);
    let summary_csv = summary_csv.text().expect("summary CSV should be text");
    assert!(
        summary_csv.contains("group,key_name,model,team_id,project_id,user_id,budget_id,requests")
    );
    assert!(summary_csv.contains("by_key,team-a,"));

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
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: vec![runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "team-a".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: Some("team-a".to_string()),
            project_id: Some("project-a".to_string()),
            user_id: Some("alice@example.com".to_string()),
            budget_id: Some("budget-a".to_string()),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(virtual_token),
            allowed_models: vec!["gpt-5.4".to_string()],
            budget_microusd: Some(1_000_000),
            request_budget: Some(5),
            rpm_limit: Some(5),
            tpm_limit: Some(1_000),
        }],
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should restart");
    let restarted_usage = client
        .get(format!(
            "http://{}/v1/prodex/gateway/usage",
            restarted.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("admin usage request after restart should be sent");
    assert_eq!(restarted_usage.status().as_u16(), 200);
    let restarted_usage: serde_json::Value = restarted_usage
        .json()
        .expect("restarted usage response should be json");
    assert_eq!(restarted_usage["keys"][0]["usage"]["requests_total"], 1);
}

#[test]
fn gateway_budget_id_request_budget_is_shared_across_virtual_keys() {
    let root = temp_root("gateway-budget-id-request-budget");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(1);
    let alpha_token = "alpha-token";
    let beta_token = "beta-token";
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
        gateway_virtual_keys: vec![
            runtime_proxy_crate::RuntimeGatewayVirtualKey {
                name: "alpha".to_string(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: Some("shared-budget".to_string()),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    alpha_token,
                ),
                allowed_models: Vec::new(),
                budget_microusd: None,
                request_budget: Some(1),
                rpm_limit: None,
                tpm_limit: None,
            },
            runtime_proxy_crate::RuntimeGatewayVirtualKey {
                name: "beta".to_string(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: Some("shared-budget".to_string()),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(beta_token),
                allowed_models: Vec::new(),
                budget_microusd: None,
                request_budget: Some(1),
                rpm_limit: None,
                tpm_limit: None,
            },
        ],
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");
    let client = reqwest::blocking::Client::new();

    let accepted = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(alpha_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "alpha"}))
        .send()
        .expect("first gateway request should be sent");
    assert_eq!(accepted.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive first request");

    let rejected = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(beta_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "beta"}))
        .send()
        .expect("second gateway request should be sent");
    assert_eq!(rejected.status().as_u16(), 403);
    let rejected: serde_json::Value = rejected.json().expect("rejection should be json");
    assert_eq!(rejected["error"]["code"], "request_budget_exceeded");
}
