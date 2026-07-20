use super::super::gemini_rewrite::{RuntimeGeminiOAuthProfileAuth, RuntimeGeminiProviderAuth};
use super::{
    AppState, RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewaySsoConfig, RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, TestUpstream, app_paths_for_root,
    runtime_gateway_test_admin_token, start_runtime_gateway_rewrite_proxy, temp_root,
};
use std::collections::BTreeMap;
use std::fs;
use std::time::Duration;

fn strict_policy() -> prodex_provider_core::ProviderRequestConstraintPolicy {
    prodex_provider_core::ProviderRequestConstraintPolicy {
        enabled: true,
        unknown_context: prodex_provider_core::ProviderUnknownContextPolicy::Reject,
        safe_window_tokens: 128_000,
        oversized_output: prodex_provider_core::ProviderOversizedOutputPolicy::Reject,
    }
}

#[test]
fn strict_live_route_matches_explain_preserves_alias_governance_and_traces_rejection() {
    let root = temp_root("gateway-request-constraints-live");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(1);
    let alias = runtime_proxy_crate::RuntimeGatewayRouteAlias {
        alias: "prodex-route".to_string(),
        models: vec!["unknown-model".to_string(), "gpt-5.4".to_string()],
        strategy: runtime_proxy_crate::RuntimeGatewayRouteStrategy::Fallback,
        model_metrics: BTreeMap::new(),
    };
    let proxy = start_runtime_gateway_rewrite_proxy(
        RuntimeLocalRewriteProxyStartOptions {
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
            gateway_admin_tokens: vec![runtime_gateway_test_admin_token("admin-token")],
            gateway_sso: RuntimeGatewaySsoConfig::default(),
            gateway_state_store: RuntimeGatewayStateStore::file(&paths),
            gateway_virtual_keys: vec![runtime_proxy_crate::RuntimeGatewayVirtualKey {
                name: "data-key".to_string(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    "data-token",
                ),
                allowed_models: vec!["prodex-route".to_string()],
                budget_microusd: None,
                request_budget: None,
                rpm_limit: None,
                tpm_limit: None,
            }],
            gateway_route_aliases: vec![alias],
            gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
                blocked_keywords: vec!["blocked-input".to_string()],
                ..Default::default()
            },
            gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
            gateway_call_id_header: Some("x-prodex-call-id".to_string()),
            gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        },
        strict_policy(),
    )
    .expect("strict gateway should start");
    let client = reqwest::blocking::Client::new();
    let explain: serde_json::Value = client
        .post(format!(
            "http://{}/v1/prodex/gateway/routes/explain",
            proxy.listen_addr
        ))
        .bearer_auth("admin-token")
        .json(&serde_json::json!({
            "endpoint": "responses",
            "requested_model": "prodex-route",
            "request": {"input":"hi"},
            "diagnostic_seed": 1
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(explain["resolved_model"], "gpt-5.4");
    assert_eq!(explain["candidate_matrix"][0]["eligibility"], "rejected");

    let response = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth("data-token")
        .json(&serde_json::json!({"model":"prodex-route","input":"hi"}))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let upstream_body: serde_json::Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(upstream_body["model"], "gpt-5.4");

    let denied = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth("data-token")
        .json(&serde_json::json!({
            "model":"prodex-route",
            "input":"blocked-input"
        }))
        .send()
        .unwrap();
    assert_eq!(denied.status().as_u16(), 403);

    let log = fs::read_to_string(&proxy.log_path).unwrap();
    assert_eq!(log.matches("route_decision").count(), 2);
    assert!(log.contains("gpt-5.4"));
    assert!(log.contains("governance"));
    assert!(log.contains("rejected"));
    assert!(log.contains("call_id=prodex-") || log.contains("call_id\":\"prodex-"));
}

#[test]
fn strict_session_owner_remembers_the_concrete_model_that_actually_succeeded() {
    let root = temp_root("gateway-request-constraints-fallback-owner");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_with_statuses(
        &[500, 200, 200],
        "application/json",
        r#"{"id":"chatcmpl-test","object":"chat.completion","created":1,"model":"deepseek-v4-flash","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#,
    );
    let upstream_base_url = format!("http://{}/v1", upstream.addr);
    let proxy = start_runtime_gateway_rewrite_proxy(
        RuntimeLocalRewriteProxyStartOptions {
            paths: &paths,
            state: &AppState::default(),
            upstream_base_url: upstream_base_url.clone(),
            provider: RuntimeLocalRewriteProviderOptions::DeepSeek {
                api_keys: vec!["upstream-key".to_string()],
                strict_tools: false,
                beta_base_url: upstream_base_url,
                web_search_mode: super::super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode::Off,
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
                name: "data-key".to_string(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    "data-token",
                ),
                allowed_models: vec!["auto".to_string()],
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
        },
        strict_policy(),
    )
    .expect("strict DeepSeek gateway should start");
    let client = reqwest::blocking::Client::new();
    for _ in 0..2 {
        let response = client
            .post(format!("http://{}/v1/responses", proxy.listen_addr))
            .bearer_auth("data-token")
            .header("session_id", "constraint-session")
            .json(&serde_json::json!({"model":"auto","input":"hi"}))
            .send()
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
    }

    let models = (0..3)
        .map(|_| {
            let body = upstream
                .body_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["model"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        models,
        vec![
            "deepseek-v4-pro".to_string(),
            "deepseek-v4-flash".to_string(),
            "deepseek-v4-flash".to_string(),
        ]
    );
}

#[test]
fn gateway_provider_history_is_scoped_to_virtual_key() {
    let root = temp_root("gateway-provider-history-scope");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n_with_response_body(
        3,
        r#"{"id":"chatcmpl-shared","object":"chat.completion","created":1,"model":"deepseek-chat","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#,
    );
    let upstream_base_url = format!("http://{}/v1", upstream.addr);
    let virtual_key = |name: &str, token: &str| runtime_proxy_crate::RuntimeGatewayVirtualKey {
        name: name.to_string(),
        tenant_id: None,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token),
        allowed_models: vec!["deepseek-chat".to_string()],
        budget_microusd: None,
        request_budget: None,
        rpm_limit: None,
        tpm_limit: None,
    };
    let proxy = start_runtime_gateway_rewrite_proxy(
        RuntimeLocalRewriteProxyStartOptions {
            paths: &paths,
            state: &AppState::default(),
            upstream_base_url: upstream_base_url.clone(),
            provider: RuntimeLocalRewriteProviderOptions::DeepSeek {
                api_keys: vec!["upstream-key".to_string()],
                strict_tools: false,
                beta_base_url: upstream_base_url,
                web_search_mode: super::super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode::Off,
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
                virtual_key("key-a", "token-a"),
                virtual_key("key-b", "token-b"),
            ],
            gateway_route_aliases: Vec::new(),
            gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
            gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
            gateway_call_id_header: None,
            gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        },
        strict_policy(),
    )
    .expect("strict DeepSeek gateway should start");
    let client = reqwest::blocking::Client::new();
    let send = |token: &str, input: &str, previous_response_id: Option<&str>| {
        let mut body = serde_json::json!({"model": "deepseek-chat", "input": input});
        if let Some(previous_response_id) = previous_response_id {
            body["previous_response_id"] = previous_response_id.into();
        }
        client
            .post(format!("http://{}/v1/responses", proxy.listen_addr))
            .bearer_auth(token)
            .json(&body)
            .send()
            .expect("gateway request should complete")
    };

    let first: serde_json::Value = send("token-a", "first-a", None).json().unwrap();
    let response_id = first["id"].as_str().expect("response id should exist");
    assert_eq!(response_id, "chatcmpl-shared");
    assert_eq!(
        send("token-a", "followup-a", Some(response_id))
            .status()
            .as_u16(),
        200
    );
    assert_eq!(
        send("token-b", "followup-b", Some(response_id))
            .status()
            .as_u16(),
        200
    );

    let request_bodies = (0..3)
        .map(|_| {
            serde_json::from_slice::<serde_json::Value>(
                &upstream
                    .body_rx
                    .recv_timeout(Duration::from_secs(2))
                    .expect("upstream request should be captured"),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let same_key_messages = request_bodies[1]["messages"].as_array().unwrap();
    assert!(
        same_key_messages
            .iter()
            .any(|message| message["content"] == "first-a")
    );
    assert!(
        same_key_messages
            .iter()
            .any(|message| message["content"] == "followup-a")
    );

    let other_key_messages = request_bodies[2]["messages"].as_array().unwrap();
    assert_eq!(other_key_messages.len(), 1);
    assert_eq!(other_key_messages[0]["content"], "followup-b");
}

#[test]
fn strict_gemini_oauth_session_remembers_the_concrete_model_that_succeeded() {
    let root = temp_root("gateway-request-constraints-gemini-owner");
    let paths = app_paths_for_root(root.clone());
    let upstream = TestUpstream::start_with_statuses(
        &[500, 200, 200],
        "application/json",
        r#"{"responseId":"resp_test","modelVersion":"gemini-3.1-pro-preview","candidates":[{"content":{"parts":[{"text":"ok"}]},"finishReason":"STOP"}]}"#,
    );
    let endpoint = format!("http://{}/v1internal", upstream.addr);
    let _endpoint = crate::TestEnvVarGuard::set("PRODEX_GEMINI_CODE_ASSIST_ENDPOINT", &endpoint);
    let proxy = start_runtime_gateway_rewrite_proxy(
        RuntimeLocalRewriteProxyStartOptions {
            paths: &paths,
            state: &AppState::default(),
            upstream_base_url: endpoint,
            provider: RuntimeLocalRewriteProviderOptions::Gemini {
                auth: RuntimeGeminiProviderAuth::OAuthProfiles {
                    profiles: vec![RuntimeGeminiOAuthProfileAuth {
                        profile_name: "profile-a".to_string(),
                        codex_home: root.join("profile-a"),
                        email: None,
                        access_token: "oauth-token".to_string(),
                        project_id: Some("project-a".to_string()),
                    }],
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
                name: "data-key".to_string(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    "data-token",
                ),
                allowed_models: vec!["auto".to_string()],
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
        },
        strict_policy(),
    )
    .expect("strict Gemini gateway should start");
    let client = reqwest::blocking::Client::new();
    for _ in 0..2 {
        let response = client
            .post(format!("http://{}/v1/responses", proxy.listen_addr))
            .bearer_auth("data-token")
            .header("session_id", "constraint-session")
            .json(&serde_json::json!({"model":"auto","input":"hi"}))
            .send()
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
    }

    let models = (0..3)
        .map(|_| {
            let body = upstream
                .body_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["model"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        models,
        vec![
            "gemini-3-pro-preview".to_string(),
            "gemini-3.1-pro-preview".to_string(),
            "gemini-3.1-pro-preview".to_string(),
        ]
    );
}
