use super::super::gemini_rewrite::RuntimeGeminiProviderAuth;
use super::*;
use crate::TestEnvVarGuard;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;
use tiny_http::{Response as TinyResponse, Server as TinyServer};

fn start_guardrail_webhook_response(body: &'static str) -> std::net::SocketAddr {
    let server = TinyServer::http("127.0.0.1:0").expect("guardrail webhook should bind");
    let addr = server
        .server_addr()
        .to_ip()
        .expect("guardrail webhook should expose TCP addr");
    thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let _ = request.respond(TinyResponse::from_string(body).with_status_code(200));
        }
    });
    addr
}

#[test]
fn gateway_realtime_websocket_requires_virtual_key_auth() {
    let root = temp_root("gateway-realtime-websocket-auth");
    let paths = app_paths_for_root(root);
    let virtual_token = "team-a-token";
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

    let response = reqwest::blocking::Client::new()
        .get(format!("http://{}/v1/realtime", proxy.listen_addr))
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header(
            "Sec-WebSocket-Key",
            ["dGhl", "IHNhbXBs", "ZSBub25jZQ=="].concat(),
        )
        .header("Sec-WebSocket-Version", "13")
        .send()
        .expect("websocket handshake request should be sent");
    assert_eq!(response.status().as_u16(), 401);
    let body: serde_json::Value = response.json().expect("error response should be json");
    assert_eq!(body["error"]["code"], "invalid_gateway_key");
}

#[test]
fn gateway_virtual_key_auth_rejects_before_reading_request_body() {
    let root = temp_root("gateway-header-auth");
    let paths = app_paths_for_root(root);
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:9/v1".to_string(),
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
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("team-a-token"),
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

    let mut stream = TcpStream::connect(proxy.listen_addr).expect("gateway should accept TCP");
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    write!(
        stream,
        "POST /v1/responses HTTP/1.1\r\nHost: {}\r\nContent-Length: 1048576\r\n\r\n",
        proxy.listen_addr
    )
    .expect("request headers should write");
    let mut response = [0_u8; 4096];
    let read = stream
        .read(&mut response)
        .expect("gateway should reject without waiting for the declared body");
    let response = String::from_utf8_lossy(&response[..read]);

    assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    assert!(response.contains("invalid_gateway_key"), "{response}");
}

#[test]
fn gateway_admin_auth_rejects_before_reading_request_body() {
    let root = temp_root("gateway-admin-header-auth");
    let paths = app_paths_for_root(root);
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:9/v1".to_string(),
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
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("admin-token"),
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

    let mut stream = TcpStream::connect(proxy.listen_addr).expect("gateway should accept TCP");
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    write!(
        stream,
        "POST /v1/prodex/gateway/keys HTTP/1.1\r\nHost: {}\r\nContent-Length: 1048576\r\n\r\n",
        proxy.listen_addr
    )
    .expect("request headers should write");
    let mut response = [0_u8; 4096];
    let read = stream
        .read(&mut response)
        .expect("gateway should reject without waiting for the declared body");
    let response = String::from_utf8_lossy(&response[..read]);

    assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    assert!(response.contains("invalid_admin_token"), "{response}");
}

#[test]
fn gateway_guardrail_webhook_fail_closed_blocks_missing_allow_field() {
    let root = temp_root("gateway-guardrail-webhook-missing-allow");
    let paths = app_paths_for_root(root);
    let webhook_addr = start_guardrail_webhook_response("{}");
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
}

#[test]
fn gateway_virtual_key_usage_is_persisted_and_visible_to_admin_endpoint() {
    let root = temp_root("gateway-virtual-key-usage");
    let audit_dir = root.join("audit");
    fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
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
    let rejected_root_token = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "root token must not bypass data-plane virtual-key accounting"
        }))
        .send()
        .expect("gateway root token request should be sent");
    assert_eq!(rejected_root_token.status().as_u16(), 401);
    let rejected_root_token_body: serde_json::Value = rejected_root_token
        .json()
        .expect("rejected root token response should be json");
    assert_eq!(
        rejected_root_token_body["error"]["code"],
        "invalid_gateway_key"
    );
    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("invalid virtual-key auth should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"auth_failed""#));
    assert!(audit_log.contains(r#""reason":"invalid_gateway_key""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(admin_token));
    assert!(!audit_log.contains(virtual_token));

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
    let call_id = response
        .headers()
        .get("x-prodex-call-id")
        .and_then(|value| value.to_str().ok())
        .expect("gateway response should include call ID")
        .to_string();
    let call_id_uuid = call_id
        .strip_prefix("prodex-")
        .expect("call ID should keep prodex prefix")
        .parse::<prodex_domain::CallId>()
        .expect("call ID should use typed UUIDv7 value");
    assert_eq!(
        call_id_uuid.as_uuid().get_version_num(),
        7,
        "call ID should not use process-local request sequence: {call_id}"
    );
    let upstream_body = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive gateway request");
    assert!(String::from_utf8_lossy(&upstream_body).contains("gpt-5.4"));

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
    assert_eq!(
        metrics
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    assert_eq!(
        metrics
            .headers()
            .get("x-content-type-options")
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );
    let metrics = metrics.text().expect("metrics response should be text");
    assert!(metrics.contains("prodex_gateway_virtual_key_requests_total"));
    assert!(metrics.contains("key_hash=\""));
    assert!(metrics.contains("source=\"policy\""));
    assert!(metrics.contains("tenant_scoped=\"true\""));
    assert!(metrics.contains("team_scoped=\"true\""));
    assert!(metrics.contains("project_scoped=\"true\""));
    assert!(metrics.contains("user_scoped=\"true\""));
    assert!(metrics.contains("budget_scoped=\"true\""));
    assert!(!metrics.contains("key=\"team-a\""));
    assert!(!metrics.contains("tenant_id=\"tenant-a\""));
    assert!(!metrics.contains("user_id=\"alice@example.com\""));

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
    wait_for_ledger_file_key_response_status(
        &root.join("gateway-billing-ledger.jsonl"),
        "team-a",
        200,
    );
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
    assert_eq!(ledger["records"][0]["call_id"], call_id);
    assert_eq!(ledger["records"][0]["tenant_id"], "tenant-a");
    assert_eq!(ledger["records"][0]["team_id"], "team-a");
    assert_eq!(ledger["records"][0]["project_id"], "project-a");
    assert_eq!(ledger["records"][0]["user_id"], "alice@example.com");
    assert_eq!(ledger["records"][0]["budget_id"], "budget-a");
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
    assert!(ledger_csv.contains(
        "call_id,request_id,key_name,tenant_id,team_id,project_id,user_id,budget_id,model"
    ));
    assert!(ledger_csv.contains(&format!("{call_id},prodex-")));
    assert!(
        ledger_csv.contains("team-a,tenant-a,team-a,project-a,alice@example.com,budget-a,gpt-5.4")
    );
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
    assert_eq!(summary["by_tenant"][0]["tenant_id"], "tenant-a");
    assert_eq!(summary["by_team"][0]["team_id"], "team-a");
    assert_eq!(summary["by_project"][0]["project_id"], "project-a");
    assert_eq!(summary["by_user"][0]["user_id"], "alice@example.com");
    assert_eq!(summary["by_budget"][0]["budget_id"], "budget-a");
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
        summary_csv.contains(
            "group,key_name,model,tenant_id,team_id,project_id,user_id,budget_id,requests"
        )
    );
    assert!(summary_csv.contains("by_key,team-a,"));
    assert!(summary_csv.contains("by_tenant,,,tenant-a,"));

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
fn gateway_virtual_key_cannot_authenticate_admin_endpoint() {
    let root = temp_root("gateway-virtual-key-admin-denied");
    let paths = app_paths_for_root(root);
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
            tenant_id: Some("tenant-a".to_string()),
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

    let denied = reqwest::blocking::Client::new()
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(virtual_token)
        .send()
        .expect("admin request with virtual key should be sent");
    assert_eq!(denied.status().as_u16(), 403);
}

#[test]
fn gateway_admin_token_cannot_bypass_virtual_key_data_plane_auth() {
    let root = temp_root("gateway-admin-token-data-plane-denied");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-token";
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
        gateway_virtual_keys: vec![runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "team-a".to_string(),
            tenant_id: Some("tenant-a".to_string()),
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

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "admin token"}))
        .send()
        .expect("gateway admin token data-plane request should be sent");
    assert_eq!(response.status().as_u16(), 401);
    assert_eq!(
        response.json::<serde_json::Value>().unwrap()["error"]["code"],
        "invalid_gateway_key"
    );
}

#[test]
fn gateway_usage_persistence_failure_logs_stable_error_without_path_or_token() {
    let root = temp_root("gateway-usage-persistence-log-redaction");
    let paths = app_paths_for_root(root.clone());
    let upstream = TestUpstream::start();
    let virtual_token = "team-a-usage-log-redaction-token";
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

    fs::create_dir_all(root.join("gateway-virtual-key-usage.json"))
        .expect("usage path directory should be created after startup");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(virtual_token)
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "usage persistence failure should be redacted"
        }))
        .send()
        .expect("gateway request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let mut runtime_log = String::new();
    for _ in 0..50 {
        runtime_log = fs::read_to_string(&proxy.log_path).unwrap_or_default();
        if runtime_log.contains("gateway_virtual_key_usage_save_failed") {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(runtime_log.contains("gateway_virtual_key_usage_save_failed"));
    assert!(runtime_log.contains("error_kind=gateway_usage_persistence_failed"));
    assert!(!runtime_log.contains("gateway-virtual-key-usage.json"));
    assert!(!runtime_log.contains(&root.display().to_string()));
    assert!(!runtime_log.contains("Is a directory"));
    assert!(!runtime_log.contains(virtual_token));
}

#[test]
fn gateway_upstream_transport_failure_uses_stable_response_without_endpoint_details() {
    let root = temp_root("gateway-upstream-transport-failure");
    let paths = app_paths_for_root(root);
    let closed_listener =
        TcpListener::bind("127.0.0.1:0").expect("closed upstream listener should bind");
    let closed_addr = closed_listener
        .local_addr()
        .expect("closed upstream listener should expose addr");
    drop(closed_listener);
    let virtual_token = "team-a-upstream-failure-token";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{closed_addr}/v1"),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: true,
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

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(virtual_token)
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "transport failure should be redacted"
        }))
        .send()
        .expect("gateway request should be sent");
    assert_eq!(response.status().as_u16(), 502);
    let body = response.text().expect("failure body should be text");
    assert_eq!(body, "upstream request failed");
    assert!(!body.contains("127.0.0.1"));
    assert!(!body.contains(&closed_addr.port().to_string()));
    assert!(!body.contains("Connection refused"));
    assert!(!body.contains(virtual_token));
}

#[test]
fn gateway_virtual_key_token_is_not_forwarded_to_openai_passthrough_upstream() {
    let root = temp_root("gateway-virtual-key-no-upstream-leak");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let virtual_token = "team-a-token";
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
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(virtual_token)
        .header("ChatGPT-Account-Id", "acct-client")
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "hello"
        }))
        .send()
        .expect("gateway request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive gateway request");
    assert!(
        headers.iter().all(|(name, value)| {
            !name.eq_ignore_ascii_case("authorization") || value != "Bearer team-a-token"
        }),
        "gateway virtual key must not be forwarded as upstream Authorization: {headers:?}"
    );
    assert!(
        headers
            .iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("chatgpt-account-id")),
        "gateway virtual key must not forward client ChatGPT-Account-Id: {headers:?}"
    );
}

#[test]
fn gateway_default_bearer_token_is_not_forwarded_to_openai_passthrough_upstream() {
    let root = temp_root("gateway-default-token-no-upstream-leak");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
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

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .header("ChatGPT-Account-Id", "acct-client")
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "hello"
        }))
        .send()
        .expect("gateway request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive gateway request");
    assert!(
        headers.iter().all(|(name, value)| {
            !name.eq_ignore_ascii_case("authorization") || value != "Bearer gateway-token"
        }),
        "gateway bearer token must not be forwarded as upstream Authorization: {headers:?}"
    );
    assert!(
        headers
            .iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("chatgpt-account-id")),
        "gateway bearer token must not forward client ChatGPT-Account-Id: {headers:?}"
    );
}

#[test]
fn gateway_disabled_last_virtual_key_does_not_open_gateway() {
    let root = temp_root("gateway-disabled-last-key");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(1);
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
    let client = reqwest::blocking::Client::new();

    let created = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"name": "alpha"}))
        .send()
        .expect("admin create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);

    let disabled = client
        .patch(format!(
            "http://{}/v1/prodex/gateway/keys/alpha",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("admin disable key request should be sent");
    assert_eq!(disabled.status().as_u16(), 200);

    let rejected = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "must not pass"}))
        .send()
        .expect("unauthenticated gateway request should be sent");
    assert_eq!(rejected.status().as_u16(), 401);
    let rejected: serde_json::Value = rejected.json().expect("rejection should be json");
    assert_eq!(rejected["error"]["code"], "invalid_gateway_key");
}

#[test]
fn openai_passthrough_provider_api_key_drops_client_account_id() {
    let root = temp_root("openai-passthrough-provider-key-no-client-account");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
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
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth("client-token")
        .header("ChatGPT-Account-Id", "acct-client")
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "hello"
        }))
        .send()
        .expect("gateway request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive passthrough request");
    assert!(
        headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("authorization") && value == "Bearer upstream-key"
        }),
        "provider API key should replace client Authorization: {headers:?}"
    );
    assert!(
        headers
            .iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("chatgpt-account-id")),
        "provider API key must not forward client ChatGPT-Account-Id: {headers:?}"
    );
}

#[test]
fn openai_passthrough_preserves_client_authorization_when_gateway_auth_is_disabled() {
    let root = temp_root("openai-passthrough-client-auth");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
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
        gateway_auth_token_hash: None,
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

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth("upstream-user-token")
        .header("ChatGPT-Account-Id", "acct-upstream")
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "hello"
        }))
        .send()
        .expect("gateway request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive passthrough request");
    assert!(
        headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("authorization") && value == "Bearer upstream-user-token"
        }),
        "non-gateway client Authorization should remain passthrough: {headers:?}"
    );
    assert!(
        headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("chatgpt-account-id") && value == "acct-upstream"
        }),
        "non-gateway client ChatGPT-Account-Id should remain passthrough: {headers:?}"
    );
}

#[test]
fn gateway_budget_id_request_budget_is_shared_across_virtual_keys() {
    let root = temp_root("gateway-budget-id-request-budget");
    let audit_dir = root.join("audit");
    fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
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

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("request-budget denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"authorization_denied""#));
    assert!(audit_log.contains(r#""reason":"request_budget_exceeded""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(alpha_token));
    assert!(!audit_log.contains(beta_token));
}

#[test]
fn gateway_rejects_oversized_request_body_before_upstream() {
    let _limit = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES", "64");
    let root = temp_root("gateway-request-body-limit");
    let audit_dir = root.join("audit");
    std::fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(1);
    let virtual_token = "body-limit-token";
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
            name: "team-body-limit".to_string(),
            tenant_id: Some("tenant-body-limit".to_string()),
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

    let oversized_input = "x".repeat(256);
    let rejected = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(virtual_token)
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": oversized_input
        }))
        .send()
        .expect("oversized gateway request should be sent");

    let status = rejected.status().as_u16();
    let rejected = rejected.text().expect("rejection should be text");
    assert_eq!(status, 413, "body={rejected}");
    assert!(rejected.contains("proxied request body is too large"));
    assert!(
        upstream
            .body_rx
            .recv_timeout(Duration::from_millis(200))
            .is_err()
    );

    let audit_log = std::fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("oversized request rejection should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"request_body_too_large""#));
    assert!(audit_log.contains(r#""reason":"request_body_too_large""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(virtual_token));
    assert!(!audit_log.contains(&oversized_input));

    let runtime_log =
        std::fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("local_rewrite_request_body_too_large"));
    assert!(runtime_log.contains("reason=request_body_too_large"));
    assert!(!runtime_log.contains(virtual_token));
    assert!(!runtime_log.contains(&oversized_input));
}

#[test]
fn gateway_data_plane_legacy_token_auth_failures_are_audited_without_token_leakage() {
    let root = temp_root("gateway-data-plane-auth-audit");
    let audit_dir = root.join("audit");
    fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let gateway_token = "gateway-data-plane-token";
    let invalid_token = "wrong-gateway-data-plane-token";
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
    let responses_url = format!("http://{}/v1/responses", proxy.listen_addr);
    let missing = client
        .post(&responses_url)
        .json(&serde_json::json!({"model":"gpt-5","input":"hello"}))
        .send()
        .expect("missing auth request should be sent");
    assert_eq!(missing.status().as_u16(), 401);

    let invalid = client
        .post(&responses_url)
        .bearer_auth(invalid_token)
        .json(&serde_json::json!({"model":"gpt-5","input":"hello"}))
        .send()
        .expect("invalid auth request should be sent");
    assert_eq!(invalid.status().as_u16(), 401);

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("data-plane auth failures should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"auth_failed""#));
    assert!(audit_log.contains(r#""reason":"missing_or_invalid_gateway_bearer_token""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(gateway_token));
    assert!(!audit_log.contains(invalid_token));
}

#[test]
fn gateway_virtual_key_model_policy_denials_are_audited_without_token_leakage() {
    let root = temp_root("gateway-virtual-key-model-policy-audit");
    let audit_dir = root.join("audit");
    fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let virtual_token = "model-policy-token";
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
            name: "model-policy-key".to_string(),
            tenant_id: Some("tenant-a".to_string()),
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
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let client = reqwest::blocking::Client::new();
    let denied = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(virtual_token)
        .json(&serde_json::json!({"model":"gpt-4.1","input":"hello"}))
        .send()
        .expect("model-denied request should be sent");
    assert_eq!(denied.status().as_u16(), 403);
    let denied: serde_json::Value = denied.json().expect("denied response should be json");
    assert_eq!(denied["error"]["code"], "model_not_allowed");

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("model policy denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"authorization_denied""#));
    assert!(audit_log.contains(r#""reason":"model_not_allowed""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(virtual_token));
}

#[test]
fn gateway_virtual_key_rpm_limit_denials_are_audited_without_token_leakage() {
    let root = temp_root("gateway-virtual-key-rpm-audit");
    let audit_dir = root.join("audit");
    fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(1);
    let virtual_token = "rpm-policy-token";
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
            name: "rpm-policy-key".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(virtual_token),
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget: None,
            rpm_limit: Some(1),
            tpm_limit: None,
        }],
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let client = reqwest::blocking::Client::new();
    let url = format!("http://{}/v1/responses", proxy.listen_addr);
    let accepted = client
        .post(&url)
        .bearer_auth(virtual_token)
        .json(&serde_json::json!({"model":"gpt-5","input":"hello"}))
        .send()
        .expect("first rpm-limited request should be sent");
    assert_eq!(accepted.status().as_u16(), 200);

    let denied = client
        .post(&url)
        .bearer_auth(virtual_token)
        .json(&serde_json::json!({"model":"gpt-5","input":"hello again"}))
        .send()
        .expect("second rpm-limited request should be sent");
    assert_eq!(denied.status().as_u16(), 429);
    let denied: serde_json::Value = denied.json().expect("denied response should be json");
    assert_eq!(denied["error"]["code"], "rpm_limit_exceeded");

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("RPM denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"authorization_denied""#));
    assert!(audit_log.contains(r#""reason":"rpm_limit_exceeded""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(virtual_token));
}

#[test]
fn gateway_virtual_key_tpm_limit_denials_are_audited_without_token_leakage() {
    let root = temp_root("gateway-virtual-key-tpm-audit");
    let audit_dir = root.join("audit");
    fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let virtual_token = "tpm-policy-token";
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
            name: "tpm-policy-key".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(virtual_token),
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: Some(0),
        }],
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let client = reqwest::blocking::Client::new();
    let denied = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(virtual_token)
        .json(&serde_json::json!({"model":"gpt-5","input":"hello"}))
        .send()
        .expect("tpm-limited request should be sent");
    assert_eq!(denied.status().as_u16(), 429);
    let denied: serde_json::Value = denied.json().expect("denied response should be json");
    assert_eq!(denied["error"]["code"], "tpm_limit_exceeded");

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("TPM denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"authorization_denied""#));
    assert!(audit_log.contains(r#""reason":"tpm_limit_exceeded""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(virtual_token));
}

#[test]
fn gateway_virtual_key_spend_budget_denials_are_audited_without_token_leakage() {
    let root = temp_root("gateway-virtual-key-spend-budget-audit");
    let audit_dir = root.join("audit");
    fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let virtual_token = "spend-budget-token";
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
            name: "spend-budget-key".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(virtual_token),
            allowed_models: Vec::new(),
            budget_microusd: Some(0),
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
        }],
        gateway_route_aliases: vec![runtime_proxy_crate::RuntimeGatewayRouteAlias {
            alias: "prodex-costly".to_string(),
            models: vec!["gpt-costly".to_string()],
            strategy: runtime_proxy_crate::RuntimeGatewayRouteStrategy::Fallback,
            model_metrics: std::collections::BTreeMap::from([(
                "gpt-costly".to_string(),
                runtime_proxy_crate::RuntimeGatewayRouteModelMetrics {
                    input_cost_per_million_microusd: Some(1_000_000),
                    output_cost_per_million_microusd: None,
                    latency_ms: None,
                    rpm_limit: None,
                    tpm_limit: None,
                },
            )]),
        }],
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let client = reqwest::blocking::Client::new();
    let denied = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(virtual_token)
        .json(&serde_json::json!({"model":"prodex-costly","input":"hello"}))
        .send()
        .expect("spend-budget-limited request should be sent");
    assert_eq!(denied.status().as_u16(), 403);
    let denied: serde_json::Value = denied.json().expect("denied response should be json");
    assert_eq!(denied["error"]["code"], "budget_exceeded");

    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("spend-budget denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"authorization_denied""#));
    assert!(audit_log.contains(r#""reason":"budget_exceeded""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(virtual_token));
}
