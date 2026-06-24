use super::local_rewrite::{
    RuntimeGatewayAdminRole, RuntimeGatewayAdminToken, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewayOidcConfig, RuntimeGatewaySsoConfig,
    RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, start_runtime_local_rewrite_proxy,
};
use super::local_rewrite_copilot::RuntimeCopilotProviderAuth;
use crate::AppState;
use std::time::Duration;

mod deepseek;
mod gateway_admin_auth;
mod gateway_admin_crud;
mod gateway_admin_scope;
mod gateway_admin_tenant_scope;
mod gateway_state;
mod gateway_usage;
mod model_memory;
mod support;

use support::*;

#[test]
fn copilot_transport_uses_copilot_api_headers_for_chat_completions() {
    let root = temp_root("copilot-chat-headers");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:9".to_string(),
        provider: RuntimeLocalRewriteProviderOptions::Copilot {
            auth: RuntimeCopilotProviderAuth::Profiles {
                profiles: vec![super::local_rewrite_copilot::RuntimeCopilotProfileAuth {
                    profile_name: "copilot-business".to_string(),
                    api_key: "copilot-runtime-token".to_string(),
                    api_url: format!("http://{}", upstream.addr),
                }],
            },
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
    .expect("copilot local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .header(reqwest::header::USER_AGENT, "codex-cli/0.1-test")
        .json(&serde_json::json!({
            "model": "codex",
            "stream": false,
            "messages": [
                {"role": "user", "content": "run a tool"},
                {"role": "assistant", "content": "done"}
            ]
        }))
        .send()
        .expect("copilot chat request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive Copilot headers");
    let header = |name: &str| {
        headers
            .iter()
            .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value.as_str()))
    };
    assert_eq!(
        header("authorization"),
        Some("Bearer copilot-runtime-token")
    );
    assert_eq!(header("user-agent"), Some("GitHubCopilotChat/0.26.7"));
    assert_eq!(header("copilot-integration-id"), Some("vscode-chat"));
    assert_eq!(header("editor-plugin-version"), Some("copilot-chat/0.26.7"));
    assert_eq!(header("openai-intent"), Some("conversation-panel"));
    assert_eq!(header("x-github-api-version"), Some("2025-04-01"));
    assert_eq!(
        header("x-vscode-user-agent-library-version"),
        Some("electron-fetch")
    );
    assert_eq!(header("x-initiator"), Some("agent"));
}

#[test]
fn gateway_admin_viewer_token_can_read_but_not_mutate_keys() {
    let root = temp_root("gateway-admin-rbac");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-token";
    let viewer_token = "viewer-token";
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
        gateway_admin_tokens: vec![
            RuntimeGatewayAdminToken {
                name: "admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    admin_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                allowed_key_prefixes: Vec::new(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
            },
            RuntimeGatewayAdminToken {
                name: "viewer".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    viewer_token,
                ),
                role: RuntimeGatewayAdminRole::Viewer,
                allowed_key_prefixes: Vec::new(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
            },
        ],
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

    let viewed = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(viewer_token)
        .send()
        .expect("viewer list keys request should be sent");
    assert_eq!(viewed.status().as_u16(), 200);

    let viewer_create = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(viewer_token)
        .json(&serde_json::json!({"name": "team-rbac"}))
        .send()
        .expect("viewer create key request should be sent");
    assert_eq!(viewer_create.status().as_u16(), 403);
    assert_eq!(
        viewer_create.json::<serde_json::Value>().unwrap()["error"]["code"],
        "gateway_admin_role_forbidden"
    );

    let created = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"name": "team-rbac"}))
        .send()
        .expect("admin create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);

    let viewer_patch = client
        .patch(format!(
            "http://{}/v1/prodex/gateway/keys/team-rbac",
            proxy.listen_addr
        ))
        .bearer_auth(viewer_token)
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("viewer patch key request should be sent");
    assert_eq!(viewer_patch.status().as_u16(), 403);

    let viewer_delete = client
        .delete(format!(
            "http://{}/v1/prodex/gateway/keys/team-rbac",
            proxy.listen_addr
        ))
        .bearer_auth(viewer_token)
        .send()
        .expect("viewer delete key request should be sent");
    assert_eq!(viewer_delete.status().as_u16(), 403);

    let metrics = client
        .get(format!(
            "http://{}/v1/prodex/gateway/metrics",
            proxy.listen_addr
        ))
        .bearer_auth(viewer_token)
        .send()
        .expect("viewer metrics request should be sent");
    assert_eq!(metrics.status().as_u16(), 200);
}

#[test]
fn gateway_guardrail_pii_redaction_rewrites_request_before_upstream() {
    let root = temp_root("gateway-pii-redaction");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(1);
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
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
            pii_redaction: true,
            ..runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default()
        },
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "email alice@example.com card 4111-1111-1111-1111"
        }))
        .send()
        .expect("gateway request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let upstream_body = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive redacted request");
    let upstream_body = String::from_utf8(upstream_body).expect("upstream body should be utf8");
    assert!(upstream_body.contains("<redacted>"));
    assert!(!upstream_body.contains("alice@example.com"));
    assert!(!upstream_body.contains("4111-1111-1111-1111"));
}

#[test]
fn local_embeddings_only_serves_embeddings_and_rejects_generation() {
    let root = temp_root("local-embeddings-only");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let gateway_token = "gateway-token";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::LocalEmbeddingsOnly {
            embedding_model: "text-embedding-3-small".to_string(),
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

    let embeddings = client
        .post(format!("http://{}/v1/embeddings", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({
            "model": "text-embedding-3-small",
            "input": ["hello world", "hello world"]
        }))
        .send()
        .expect("embedding request should be sent");
    assert_eq!(embeddings.status().as_u16(), 200);
    let embeddings: serde_json::Value = embeddings.json().expect("embedding response is JSON");
    assert_eq!(embeddings["object"], "list");
    assert_eq!(embeddings["model"], "text-embedding-3-small");
    let first = embeddings["data"][0]["embedding"]
        .as_array()
        .expect("embedding should be an array");
    let second = embeddings["data"][1]["embedding"]
        .as_array()
        .expect("embedding should be an array");
    assert_eq!(first.len(), 1536);
    assert_eq!(first, second);

    let generation = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({
            "model": "gpt-4.1-nano",
            "input": "should not be forwarded"
        }))
        .send()
        .expect("generation request should be sent");
    assert_eq!(generation.status().as_u16(), 503);
    let generation: serde_json::Value = generation.json().expect("error response is JSON");
    assert_eq!(generation["error"]["code"], "local_embeddings_only");
}
