use super::{
    AppState, Duration, RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, TestUpstream, app_paths_for_root,
    start_runtime_local_rewrite_proxy, start_runtime_local_rewrite_proxy_with_harness, temp_root,
};
use crate::runtime_launch::proxy_startup::{
    RuntimeAnthropicProviderAuth, RuntimeDeepSeekWebSearchMode,
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

fn start_anthropic_harness_proxy(
    paths: &crate::AppPaths,
    upstream_base_url: String,
    harness: prodex_provider_core::HarnessMode,
    gateway_sso: RuntimeGatewaySsoConfig,
) -> crate::RuntimeRotationProxy {
    start_runtime_local_rewrite_proxy_with_harness(
        RuntimeLocalRewriteProxyStartOptions {
            paths,
            state: &AppState::default(),
            upstream_base_url,
            provider: RuntimeLocalRewriteProviderOptions::Anthropic {
                auth: RuntimeAnthropicProviderAuth::ApiKeys {
                    api_keys: vec!["fixture-anthropic-key".to_string()],
                },
            },
            upstream_no_proxy: false,
            smart_context_enabled: false,
            presidio_redaction_enabled: false,
            model_context_window_tokens: None,
            preferred_listen_addr: Some("127.0.0.1:0"),
            gateway_auth_token_hash: None,
            gateway_admin_tokens: Vec::new(),
            gateway_sso,
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
    .expect("Anthropic harness proxy should start")
}

fn start_deepseek_proxy(
    paths: &crate::AppPaths,
    upstream_base_url: String,
) -> crate::RuntimeRotationProxy {
    start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths,
        state: &AppState::default(),
        upstream_base_url,
        provider: RuntimeLocalRewriteProviderOptions::DeepSeek {
            api_keys: vec!["fixture-deepseek-key".to_string()],
            strict_tools: false,
            beta_base_url: "https://api.deepseek.com/beta".to_string(),
            web_search_mode: RuntimeDeepSeekWebSearchMode::Auto,
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
    })
    .expect("DeepSeek proxy should start")
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
fn evaluated_anthropic_preserves_safe_caller_metadata_through_native_transport() {
    let root = temp_root("harness-evaluated-anthropic");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n_with_response_body(
        2,
        r#"{"id":"msg_test","type":"message","role":"assistant","model":"claude-sonnet-4-6","content":[{"type":"text","text":"hello"}],"stop_reason":"end_turn","usage":{"input_tokens":3,"output_tokens":2}}"#,
    );
    let proxy = start_anthropic_harness_proxy(
        &paths,
        format!("http://{}/v1", upstream.addr),
        prodex_provider_core::HarnessMode::Evaluated,
        RuntimeGatewaySsoConfig {
            token_header: "x-auth-token".to_string(),
            user_header: "x-auth-user".to_string(),
            role_header: "x-auth-role".to_string(),
            tenant_header: "x-auth-tenant".to_string(),
            key_prefixes_header: "x-auth-prefixes".to_string(),
            ..RuntimeGatewaySsoConfig::default()
        },
    );

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth("caller-secret")
        .header("ChatGPT-Account-Id", "caller-account")
        .header(reqwest::header::USER_AGENT, "codex-cli/0.1-test")
        .header("session_id", "session-123")
        .header("x-openai-subagent", "subagent-123")
        .header("x-codex-turn-state", "turn-state-123")
        .header("x-codex-turn-metadata", "turn-metadata-123")
        .header("x-codex-beta-features", "beta-123")
        .header("x-safe-end-to-end", "keep-me")
        .header(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .header("tracestate", "prodex=test")
        .header("baggage", "tenant_tier=premium")
        .header("x-auth-token", "fixture-sso-token")
        .header("x-auth-user", "fixture-user")
        .header("x-auth-role", "fixture-role")
        .header("x-auth-tenant", "fixture-tenant")
        .header("x-auth-prefixes", "team-a-")
        .header("connection", "x-hop-by-hop")
        .header("x-hop-by-hop", "strip-me")
        .header("x-prodex-internal-test", "strip-me")
        .json(&serde_json::json!({
            "instructions": "Be concise.",
            "input": "hello",
            "stream": false
        }))
        .send()
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let response: serde_json::Value = response.json().unwrap();
    assert_eq!(response["id"], "msg_test");
    assert_eq!(
        response["output"][0]["content"][0]["text"], "hello",
        "{response}"
    );
    assert_eq!(response["usage"]["total_tokens"], 5);

    assert_eq!(
        upstream
            .path_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
        "/v1/messages"
    );
    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(
        headers
            .iter()
            .any(|(name, value)| { name == "x-api-key" && value == "fixture-anthropic-key" })
    );
    assert!(
        headers
            .iter()
            .any(|(name, value)| { name == "anthropic-version" && value == "2023-06-01" })
    );
    let header = |name: &str| {
        headers
            .iter()
            .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value.as_str()))
    };
    assert_eq!(header("user-agent"), Some("codex-cli/0.1-test"));
    for (name, value) in [
        ("session_id", "session-123"),
        ("x-openai-subagent", "subagent-123"),
        ("x-codex-turn-state", "turn-state-123"),
        ("x-codex-turn-metadata", "turn-metadata-123"),
        ("x-codex-beta-features", "beta-123"),
        ("x-safe-end-to-end", "keep-me"),
        (
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        ),
        ("tracestate", "prodex=test"),
        ("baggage", "tenant_tier=premium"),
    ] {
        assert_eq!(
            header(name),
            Some(value),
            "safe caller header was lost: {name}"
        );
    }
    for name in [
        "authorization",
        "chatgpt-account-id",
        "x-auth-token",
        "x-auth-user",
        "x-auth-role",
        "x-auth-tenant",
        "x-auth-prefixes",
        "x-hop-by-hop",
        "x-prodex-internal-test",
    ] {
        assert_eq!(
            header(name),
            None,
            "filtered header reached provider: {name}"
        );
    }
    let request: serde_json::Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(request["system"], "Be concise.");
    assert_eq!(request["messages"][0]["role"], "user");
    assert_eq!(request["messages"][0]["content"][0]["text"], "hello");
    assert_eq!(request["max_tokens"], 4096);

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "input": "continue",
            "previous_response_id": "msg_test",
            "stream": false
        }))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        upstream
            .path_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
        "/v1/messages"
    );
    let continued: serde_json::Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
    )
    .unwrap();
    assert!(continued.get("previous_response_id").is_none());
    assert!(
        continued["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| {
                message["role"] == "assistant"
                    && message["content"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|block| block["type"] == "text" && block["text"] == "hello")
            })
    );
    assert_eq!(
        continued["messages"].as_array().unwrap().last().unwrap()["content"][0]["text"],
        "continue"
    );
}

#[test]
fn deepseek_auto_web_search_uses_native_anthropic_transport() {
    let root = temp_root("deepseek-native-web-search");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_with_response_body(
        r#"{"id":"msg_search","type":"message","role":"assistant","model":"deepseek-chat","content":[{"type":"server_tool_use","id":"srv_1","name":"web_search","input":{"query":"current release"}},{"type":"web_search_tool_result","tool_use_id":"srv_1","content":[{"type":"web_search_result","url":"https://example.com/release","title":"Release"}]},{"type":"text","text":"Found it."}],"stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":2,"server_tool_use":{"web_search_requests":1}}}"#,
    );
    let proxy = start_deepseek_proxy(&paths, format!("http://{}/v1", upstream.addr));

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "deepseek-chat",
            "input": "find the current release",
            "tools": [{
                "type": "web_search_preview",
                "context_size": "high",
                "allowed_domains": ["example.com"]
            }],
            "stream": false
        }))
        .send()
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let response: serde_json::Value = response.json().unwrap();
    assert_eq!(response["output"][0]["type"], "web_search_call");
    assert_eq!(
        response["output"][0]["action"]["queries"][0],
        "current release"
    );
    assert_eq!(response["output"][1]["content"][0]["text"], "Found it.");
    assert_eq!(response["tool_usage"]["web_search"]["num_requests"], 1);

    assert_eq!(
        upstream
            .path_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
        "/anthropic/v1/messages"
    );
    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(
        headers
            .iter()
            .any(|(name, value)| name == "x-api-key" && value == "fixture-deepseek-key")
    );
    assert!(
        headers
            .iter()
            .any(|(name, value)| name == "anthropic-version" && value == "2023-06-01")
    );
    assert!(!headers.iter().any(|(name, _)| name == "authorization"));
    let request: serde_json::Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(request["tools"][0]["type"], "web_search_20250305");
    assert_eq!(request["tools"][0]["allowed_domains"][0], "example.com");
}

#[test]
fn deepseek_auto_web_search_falls_back_before_send_when_native_translation_rejects() {
    let root = temp_root("deepseek-auto-web-search-fallback");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_with_response_body(
        r#"{"id":"chat_issue_61","object":"chat.completion","created":1,"model":"deepseek-v4-flash","choices":[{"index":0,"message":{"role":"assistant","content":"safe fallback"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#,
    );
    let proxy = start_deepseek_proxy(&paths, format!("http://{}/v1", upstream.addr));

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "deepseek-v4-flash",
            "input": "continue coding",
            "response_format": {"type": "json_object"},
            "tools": [
                {"type": "web_search_preview", "context_size": "high"},
                {"type": "function", "name": "shell", "parameters": {"type": "object"}}
            ],
            "tool_choice": "none",
            "stream": false
        }))
        .send()
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let response: serde_json::Value = response.json().unwrap();
    assert_eq!(response["output"][0]["content"][0]["text"], "safe fallback");
    assert!(
        !response.to_string().contains(
            "request is incompatible with DeepSeek native Anthropic web-search translation"
        )
    );

    assert_eq!(
        upstream
            .path_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
        "/v1/chat/completions"
    );
    let request: serde_json::Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
    )
    .unwrap();
    assert!(request.get("web_search_options").is_none());
    assert_eq!(request["model"], "deepseek-v4-flash");
    assert_eq!(request["response_format"]["type"], "json_object");
    assert_eq!(request["tools"][0]["function"]["name"], "shell");
    assert!(upstream.path_rx.try_recv().is_err());
}

#[test]
fn deepseek_compact_uses_local_emulation_without_upstream_io() {
    let root = temp_root("deepseek-local-compact");
    let paths = app_paths_for_root(root);
    let proxy = start_deepseek_proxy(&paths, "http://127.0.0.1:9/v1".to_string());

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses/compact", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "retain compact context"}]
            }]
        }))
        .send()
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let response: serde_json::Value = response.json().unwrap();
    assert!(
        response["output"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("retain compact context")
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
fn minimal_harness_preserves_compact_input_for_local_emulation() {
    let root = temp_root("harness-minimal-compact");
    let paths = app_paths_for_root(root);
    let proxy = start_openai_harness_proxy(
        &paths,
        "http://127.0.0.1:9/v1".to_string(),
        prodex_provider_core::HarnessMode::Minimal,
    );
    let body = br#"{"model":"gpt-5.4","instructions":{"structured":true},"input":[{"type":"message","role":"user","content":[{"type":"input_text","text":"keep"}]}]}"#;

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses/compact", proxy.listen_addr))
        .header("content-type", "application/json")
        .body(body.to_vec())
        .send()
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let response: serde_json::Value = response.json().unwrap();
    assert!(
        response["output"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("keep")
    );
}
