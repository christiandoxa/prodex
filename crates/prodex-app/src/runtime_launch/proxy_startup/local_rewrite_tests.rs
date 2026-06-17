use super::deepseek_rewrite::{
    RuntimeDeepSeekChatSseReader, RuntimeDeepSeekConversationStore,
    runtime_deepseek_chat_request_body,
};
use super::local_rewrite::{
    RuntimeGatewayAdminRole, RuntimeGatewayAdminToken, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewayOidcConfig, RuntimeGatewaySsoConfig,
    RuntimeGatewayStateStore, RuntimeGatewayVirtualKeyUsageDelta,
    RuntimeLocalRewriteModelMemoryState, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, runtime_gateway_virtual_key_usage_apply_deltas,
    runtime_local_rewrite_model_allows_session_memory, runtime_local_rewrite_model_scope,
    start_runtime_local_rewrite_proxy,
};
use super::provider_bridge::RuntimeProviderBridgeKind;
use crate::{AppPaths, AppState, RuntimeProxyRequest};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{Cursor, Read};
use std::net::SocketAddr;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tiny_http::{Header as TinyHeader, Response as TinyResponse, Server as TinyServer};

fn env_lock() -> &'static Mutex<()> {
    static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_ENV_LOCK.get_or_init(|| Mutex::new(()))
}

struct TestEnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl TestEnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let guard = env_lock().lock().unwrap();
        let previous = std::env::var_os(key);
        // SAFETY: the shared test env lock serializes mutation and restoration.
        unsafe { std::env::set_var(key, value) };
        Self {
            key,
            previous,
            _guard: guard,
        }
    }
}

impl Drop for TestEnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            // SAFETY: the shared test env lock serializes mutation and restoration.
            unsafe { std::env::set_var(self.key, previous) };
        } else {
            // SAFETY: the shared test env lock serializes mutation and restoration.
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

fn deepseek_conversation_store() -> RuntimeDeepSeekConversationStore {
    Arc::new(Mutex::new(BTreeMap::new()))
}

#[test]
fn local_rewrite_model_memory_is_scoped_by_provider_and_session() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: vec![("session_id".to_string(), "sess-a".to_string())],
        body: Vec::new(),
    };
    let body = serde_json::to_vec(&serde_json::json!({"model": "auto"})).unwrap();
    let gemini_scope =
        runtime_local_rewrite_model_scope(RuntimeProviderBridgeKind::Gemini, &request, &body)
            .unwrap();
    let deepseek_scope =
        runtime_local_rewrite_model_scope(RuntimeProviderBridgeKind::DeepSeek, &request, &body)
            .unwrap();
    let mut memory = RuntimeLocalRewriteModelMemoryState::default();

    memory.remember_selected_model(&gemini_scope, "gemini-3.1-pro-preview");

    assert_eq!(
        memory.selected_model(&gemini_scope).as_deref(),
        Some("gemini-3.1-pro-preview")
    );
    assert_eq!(memory.selected_model(&deepseek_scope), None);
}

#[test]
fn local_rewrite_model_memory_only_overrides_auto_like_models() {
    assert!(runtime_local_rewrite_model_allows_session_memory("auto"));
    assert!(runtime_local_rewrite_model_allows_session_memory("default"));
    assert!(runtime_local_rewrite_model_allows_session_memory(""));
    assert!(!runtime_local_rewrite_model_allows_session_memory(
        "claude-sonnet-4-6"
    ));
}

#[test]
fn deepseek_request_translation_maps_responses_input_and_tools() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "stream": true,
        "instructions": "Be concise.",
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "List files"}]
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "README.md"
            }
        ],
        "tools": [
            {
                "type": "function",
                "name": "shell",
                "description": "Run shell",
                "parameters": {"type": "object"}
            },
            {"type": "web_search_preview"}
        ],
        "tool_choice": {
            "type": "function",
            "name": "shell"
        },
        "reasoning": {
            "effort": "xhigh"
        },
        "max_output_tokens": 123
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["model"], "deepseek-v4-pro");
    assert_eq!(translated["stream"], true);
    assert_eq!(translated["max_tokens"], 123);
    assert_eq!(translated["messages"][0]["role"], "system");
    assert_eq!(translated["messages"][1]["content"], "List files");
    assert_eq!(translated["messages"].as_array().unwrap().len(), 2);
    assert_eq!(translated["tools"].as_array().unwrap().len(), 1);
    assert_eq!(translated["tools"][0]["function"]["name"], "shell");
    assert!(translated.get("tool_choice").is_none());
    assert_eq!(translated["thinking"]["type"], "enabled");
    assert_eq!(translated["reasoning_effort"], "max");
}

#[test]
fn deepseek_request_translation_prepends_previous_response_history() {
    let conversations = deepseek_conversation_store();
    conversations.lock().unwrap().insert(
        "resp_prev".to_string(),
        vec![
            serde_json::json!({"role": "user", "content": "old prompt"}),
            serde_json::json!({"role": "assistant", "content": "old answer"}),
        ],
    );
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "previous_response_id": "resp_prev",
        "input": "new prompt"
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["messages"][0]["content"], "old prompt");
    assert_eq!(translated["messages"][1]["content"], "old answer");
    assert_eq!(translated["messages"][2]["content"], "new prompt");
}

#[test]
fn deepseek_sse_reader_maps_text_and_tool_calls_to_responses_events() {
    let conversations = deepseek_conversation_store();
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_1\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"shell\",\"arguments\":\"{\\\"cmd\\\":\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"ls\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        conversations,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.created"));
    assert!(output.contains("\"type\":\"response.output_text.delta\""));
    assert!(output.contains("\"delta\":\"hi\""));
    assert!(output.contains("\"type\":\"response.output_item.added\""));
    assert!(output.contains("\"type\":\"response.function_call_arguments.delta\""));
    assert!(output.contains("\"type\":\"response.output_item.done\""));
    assert!(output.contains("\"arguments\":\"{\\\"cmd\\\":\\\"rtk ls\\\"}\""));
    assert!(output.contains("event: response.completed"));
}

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
            tenant_id: None,
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
    let metrics = metrics.text().expect("metrics response should be text");
    assert!(metrics.contains("prodex_gateway_virtual_key_requests_total"));
    assert!(metrics.contains("key=\"team-a\""));
    assert!(metrics.contains("source=\"policy\""));

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
    assert!(summary_csv.contains("group,key_name,model,requests"));
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
            tenant_id: None,
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
fn gateway_admin_can_create_rotate_disable_and_delete_virtual_keys() {
    let root = temp_root("gateway-admin-crud");
    let audit_dir = root.join("audit");
    let _audit_env = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", audit_dir.to_str().unwrap());
    let paths = app_paths_for_root(root.clone());
    let upstream = TestUpstream::start_n(2);
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
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");
    let client = reqwest::blocking::Client::new();
    let openapi = client
        .get(format!(
            "http://{}/v1/prodex/gateway/openapi.json",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("OpenAPI request should be sent");
    assert_eq!(openapi.status().as_u16(), 200);
    let openapi: serde_json::Value = openapi.json().expect("OpenAPI response should be json");
    assert_eq!(openapi["openapi"], "3.1.0");
    assert!(openapi["paths"]["/v1/responses"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/keys"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/keys/{name}"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/scim/v2/Users"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/scim/v2/Users/{id}"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/ledger"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/ledger.csv"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/ledger/summary"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/ledger/summary.csv"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/metrics"].is_object());
    assert!(openapi["paths"]["/v1/prodex/gateway/admin"].is_object());

    let dashboard = client
        .get(format!(
            "http://{}/v1/prodex/gateway/admin",
            proxy.listen_addr
        ))
        .send()
        .expect("admin dashboard request should be sent");
    assert_eq!(dashboard.status().as_u16(), 200);
    assert!(
        dashboard
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .contains("text/html")
    );
    let dashboard = dashboard.text().expect("dashboard should be text");
    assert!(dashboard.contains("Prodex Gateway Admin"));
    assert!(dashboard.contains("SSO Users"));
    assert!(dashboard.contains("/scim/v2/Users"));
    assert!(dashboard.contains("/v1/prodex/gateway"));

    let unauthenticated_keys = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .send()
        .expect("unauthenticated admin API request should be sent");
    assert_eq!(unauthenticated_keys.status().as_u16(), 401);

    let created = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "name": "team-crud",
            "allowed_models": ["gpt-5.4"],
            "request_budget": 2,
            "rpm_limit": 10,
            "tpm_limit": 1000
        }))
        .send()
        .expect("create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    let created: serde_json::Value = created.json().expect("create key response should be json");
    let first_token = created["token"]
        .as_str()
        .expect("generated token should be returned once")
        .to_string();
    assert_eq!(created["key"]["source"], "admin");
    assert_eq!(created["key"]["name"], "team-crud");

    let first = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&first_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "first"}))
        .send()
        .expect("request with generated key should be sent");
    assert_eq!(first.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive first request");

    let disabled = client
        .patch(format!(
            "http://{}/v1/prodex/gateway/keys/team-crud",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("disable key request should be sent");
    assert_eq!(disabled.status().as_u16(), 200);
    assert_eq!(
        disabled.json::<serde_json::Value>().unwrap()["key"]["disabled"],
        true
    );

    let rejected = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&first_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "blocked"}))
        .send()
        .expect("request with disabled key should be sent");
    assert_eq!(rejected.status().as_u16(), 401);

    let rotated = client
        .patch(format!(
            "http://{}/v1/prodex/gateway/keys/team-crud",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"disabled": false, "rotate": true}))
        .send()
        .expect("rotate key request should be sent");
    assert_eq!(rotated.status().as_u16(), 200);
    let rotated: serde_json::Value = rotated.json().expect("rotate response should be json");
    let rotated_token = rotated["token"]
        .as_str()
        .expect("rotated token should be returned once");
    assert_ne!(rotated_token, first_token);

    let old_rejected = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&first_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "old"}))
        .send()
        .expect("request with old rotated key should be sent");
    assert_eq!(old_rejected.status().as_u16(), 401);

    let second = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(rotated_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "second"}))
        .send()
        .expect("request with rotated key should be sent");
    assert_eq!(second.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive second request");

    let deleted = client
        .delete(format!(
            "http://{}/v1/prodex/gateway/keys/team-crud",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("delete key request should be sent");
    assert_eq!(deleted.status().as_u16(), 200);

    let missing = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys/team-crud",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("get deleted key request should be sent");
    assert_eq!(missing.status().as_u16(), 404);
    let store = wait_for_json_file(&root.join("gateway-virtual-keys.json"));
    assert_eq!(store["keys"].as_array().unwrap().len(), 0);
    let audit_log = fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("gateway admin audit log should be written");
    assert!(audit_log.contains(r#""component":"gateway_admin""#));
    assert!(audit_log.contains(r#""action":"create_key""#));
    assert!(audit_log.contains(r#""action":"update_key""#));
    assert!(audit_log.contains(r#""action":"rotate_key""#));
    assert!(audit_log.contains(r#""action":"delete_key""#));
    assert!(audit_log.contains(r#""key_name":"team-crud""#));
    assert!(!audit_log.contains(&first_token));
    assert!(!audit_log.contains(rotated_token));
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
            },
            RuntimeGatewayAdminToken {
                name: "viewer".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    viewer_token,
                ),
                role: RuntimeGatewayAdminRole::Viewer,
                allowed_key_prefixes: Vec::new(),
                tenant_id: None,
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
fn gateway_admin_token_key_prefix_scope_limits_key_access() {
    let root = temp_root("gateway-admin-key-scope");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(2);
    let admin_token = "admin-token";
    let scoped_token = "scoped-token";
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
            },
            RuntimeGatewayAdminToken {
                name: "team-a-admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    scoped_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                allowed_key_prefixes: vec!["team-a-".to_string()],
                tenant_id: None,
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

    let team_a = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"name": "team-a-main"}))
        .send()
        .expect("admin create team-a key request should be sent");
    assert_eq!(team_a.status().as_u16(), 201);
    let team_a: serde_json::Value = team_a
        .json()
        .expect("team-a create response should be json");
    let team_a_token = team_a["token"]
        .as_str()
        .expect("team-a generated token should be returned")
        .to_string();

    let team_b = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"name": "team-b-main"}))
        .send()
        .expect("admin create team-b key request should be sent");
    assert_eq!(team_b.status().as_u16(), 201);
    let team_b: serde_json::Value = team_b
        .json()
        .expect("team-b create response should be json");
    let team_b_token = team_b["token"]
        .as_str()
        .expect("team-b generated token should be returned")
        .to_string();

    for (token, input) in [
        (&team_a_token, "team-a traffic"),
        (&team_b_token, "team-b traffic"),
    ] {
        let response = client
            .post(format!("http://{}/v1/responses", proxy.listen_addr))
            .bearer_auth(token)
            .json(&serde_json::json!({"model": "gpt-5.4", "input": input}))
            .send()
            .expect("gateway request should be sent");
        assert_eq!(response.status().as_u16(), 200);
        let _ = upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("upstream should receive gateway request");
    }
    wait_for_ledger_file_key_response_status(
        &paths.root.join("gateway-billing-ledger.jsonl"),
        "team-b-main",
        200,
    );

    let scoped_keys = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped key list request should be sent");
    assert_eq!(scoped_keys.status().as_u16(), 200);
    let scoped_keys: serde_json::Value = scoped_keys
        .json()
        .expect("scoped key list response should be json");
    let scoped_names = scoped_keys["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(scoped_names, vec!["team-a-main"]);

    let scoped_get_allowed = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys/team-a-main",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped get allowed key request should be sent");
    assert_eq!(scoped_get_allowed.status().as_u16(), 200);

    let scoped_get_forbidden = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys/team-b-main",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped get forbidden key request should be sent");
    assert_eq!(scoped_get_forbidden.status().as_u16(), 403);
    assert_eq!(
        scoped_get_forbidden.json::<serde_json::Value>().unwrap()["error"]["code"],
        "gateway_admin_key_scope_forbidden"
    );

    let scoped_create_forbidden = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .json(&serde_json::json!({"name": "team-b-new"}))
        .send()
        .expect("scoped forbidden create key request should be sent");
    assert_eq!(scoped_create_forbidden.status().as_u16(), 403);

    let scoped_create_allowed = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .json(&serde_json::json!({"name": "team-a-new"}))
        .send()
        .expect("scoped allowed create key request should be sent");
    assert_eq!(scoped_create_allowed.status().as_u16(), 201);

    let scoped_patch_forbidden = client
        .patch(format!(
            "http://{}/v1/prodex/gateway/keys/team-b-main",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("scoped forbidden patch key request should be sent");
    assert_eq!(scoped_patch_forbidden.status().as_u16(), 403);

    let scoped_delete_forbidden = client
        .delete(format!(
            "http://{}/v1/prodex/gateway/keys/team-b-main",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped forbidden delete key request should be sent");
    assert_eq!(scoped_delete_forbidden.status().as_u16(), 403);

    let scoped_ledger = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped ledger request should be sent");
    assert_eq!(scoped_ledger.status().as_u16(), 200);
    let scoped_ledger: serde_json::Value = scoped_ledger
        .json()
        .expect("scoped ledger response should be json");
    assert_eq!(scoped_ledger["records"].as_array().unwrap().len(), 1);
    assert_eq!(scoped_ledger["records"][0]["key_name"], "team-a-main");

    let scoped_summary = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger/summary",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped billing summary request should be sent");
    assert_eq!(scoped_summary.status().as_u16(), 200);
    let scoped_summary: serde_json::Value = scoped_summary
        .json()
        .expect("scoped billing summary response should be json");
    assert_eq!(scoped_summary["totals"]["requests"], 1);
    assert_eq!(scoped_summary["by_key"][0]["key_name"], "team-a-main");

    let scoped_metrics = client
        .get(format!(
            "http://{}/v1/prodex/gateway/metrics",
            proxy.listen_addr
        ))
        .bearer_auth(scoped_token)
        .send()
        .expect("scoped metrics request should be sent");
    assert_eq!(scoped_metrics.status().as_u16(), 200);
    let scoped_metrics = scoped_metrics
        .text()
        .expect("scoped metrics should be text");
    assert!(scoped_metrics.contains("key=\"team-a-main\""));
    assert!(scoped_metrics.contains("key=\"team-a-new\""));
    assert!(!scoped_metrics.contains("key=\"team-b-main\""));
}

#[test]
fn gateway_admin_token_tenant_scope_limits_admin_surfaces() {
    let root = temp_root("gateway-admin-tenant-scope");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(2);
    let tenant_a_token = "tenant-a-token";
    let tenant_b_token = "tenant-b-token";
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
                name: "tenant-a-admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    tenant_a_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                allowed_key_prefixes: Vec::new(),
                tenant_id: Some("tenant-a".to_string()),
            },
            RuntimeGatewayAdminToken {
                name: "tenant-b-admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    tenant_b_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                allowed_key_prefixes: Vec::new(),
                tenant_id: Some("tenant-b".to_string()),
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

    let tenant_a_key = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .json(&serde_json::json!({"name": "shared-main"}))
        .send()
        .expect("tenant-a create key request should be sent");
    assert_eq!(tenant_a_key.status().as_u16(), 201);
    let tenant_a_key: serde_json::Value = tenant_a_key.json().expect("tenant-a key json");
    assert_eq!(tenant_a_key["key"]["tenant_id"], "tenant-a");
    let tenant_a_virtual_token = tenant_a_key["token"]
        .as_str()
        .expect("tenant-a generated token")
        .to_string();

    let tenant_b_key = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_b_token)
        .json(&serde_json::json!({"name": "other-main"}))
        .send()
        .expect("tenant-b create key request should be sent");
    assert_eq!(tenant_b_key.status().as_u16(), 201);
    let tenant_b_key: serde_json::Value = tenant_b_key.json().expect("tenant-b key json");
    assert_eq!(tenant_b_key["key"]["tenant_id"], "tenant-b");
    let tenant_b_virtual_token = tenant_b_key["token"]
        .as_str()
        .expect("tenant-b generated token")
        .to_string();

    let tenant_a_cross_tenant_create = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .json(&serde_json::json!({"name": "tenant-b-forbidden", "tenant_id": "tenant-b"}))
        .send()
        .expect("tenant-a cross-tenant create request should be sent");
    assert_eq!(tenant_a_cross_tenant_create.status().as_u16(), 403);

    for (token, input) in [
        (&tenant_a_virtual_token, "tenant-a traffic"),
        (&tenant_b_virtual_token, "tenant-b traffic"),
    ] {
        let response = client
            .post(format!("http://{}/v1/responses", proxy.listen_addr))
            .bearer_auth(token)
            .json(&serde_json::json!({"model": "gpt-5.4", "input": input}))
            .send()
            .expect("gateway request should be sent");
        assert_eq!(response.status().as_u16(), 200);
        let _ = upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("upstream should receive gateway request");
    }
    wait_for_ledger_file_key_response_status(
        &paths.root.join("gateway-billing-ledger.jsonl"),
        "other-main",
        200,
    );

    let tenant_a_keys = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .send()
        .expect("tenant-a list keys request should be sent");
    assert_eq!(tenant_a_keys.status().as_u16(), 200);
    let tenant_a_keys: serde_json::Value = tenant_a_keys.json().expect("tenant-a keys json");
    let tenant_a_names = tenant_a_keys["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(tenant_a_names, vec!["shared-main"]);

    let tenant_a_get_b = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys/other-main",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .send()
        .expect("tenant-a forbidden get key request should be sent");
    assert_eq!(tenant_a_get_b.status().as_u16(), 403);

    let tenant_a_scim_user = client
        .post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .json(&serde_json::json!({"userName": "alice@example.com"}))
        .send()
        .expect("tenant-a SCIM create request should be sent");
    assert_eq!(tenant_a_scim_user.status().as_u16(), 201);
    let tenant_a_scim_user: serde_json::Value =
        tenant_a_scim_user.json().expect("tenant-a SCIM user json");
    assert_eq!(tenant_a_scim_user["tenant_id"], "tenant-a");

    let tenant_b_scim_user = client
        .post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_b_token)
        .json(&serde_json::json!({"userName": "bob@example.com"}))
        .send()
        .expect("tenant-b SCIM create request should be sent");
    assert_eq!(tenant_b_scim_user.status().as_u16(), 201);

    let tenant_a_users = client
        .get(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .send()
        .expect("tenant-a SCIM list request should be sent");
    assert_eq!(tenant_a_users.status().as_u16(), 200);
    let tenant_a_users: serde_json::Value = tenant_a_users.json().expect("tenant-a SCIM list json");
    let tenant_a_user_names = tenant_a_users["Resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|user| user["userName"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(tenant_a_user_names, vec!["alice@example.com"]);

    let tenant_a_ledger = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .send()
        .expect("tenant-a ledger request should be sent");
    assert_eq!(tenant_a_ledger.status().as_u16(), 200);
    let tenant_a_ledger: serde_json::Value = tenant_a_ledger.json().expect("tenant-a ledger json");
    assert_eq!(tenant_a_ledger["records"].as_array().unwrap().len(), 1);
    assert_eq!(tenant_a_ledger["records"][0]["key_name"], "shared-main");

    let tenant_a_metrics = client
        .get(format!(
            "http://{}/v1/prodex/gateway/metrics",
            proxy.listen_addr
        ))
        .bearer_auth(tenant_a_token)
        .send()
        .expect("tenant-a metrics request should be sent");
    assert_eq!(tenant_a_metrics.status().as_u16(), 200);
    let tenant_a_metrics = tenant_a_metrics.text().expect("tenant-a metrics text");
    assert!(tenant_a_metrics.contains("key=\"shared-main\""));
    assert!(!tenant_a_metrics.contains("key=\"other-main\""));
}

#[test]
fn gateway_sso_headers_can_authenticate_scoped_admin() {
    let root = temp_root("gateway-sso-admin");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let sso_token = "sso-proxy-token";
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
        gateway_sso: RuntimeGatewaySsoConfig {
            proxy_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                sso_token,
            )),
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            oidc: None,
            default_role: RuntimeGatewayAdminRole::Viewer,
        },
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

    let rejected = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", "wrong-token")
        .header("x-prodex-sso-user", "alice@example.com")
        .send()
        .expect("bad SSO request should be sent");
    assert_eq!(rejected.status().as_u16(), 401);

    let created = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .header("x-prodex-sso-role", "admin")
        .header("x-prodex-sso-key-prefixes", "team-a-")
        .json(&serde_json::json!({"name": "team-a-sso"}))
        .send()
        .expect("SSO admin create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);

    let forbidden = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .header("x-prodex-sso-role", "admin")
        .header("x-prodex-sso-key-prefixes", "team-a-")
        .json(&serde_json::json!({"name": "team-b-sso"}))
        .send()
        .expect("SSO admin forbidden create key request should be sent");
    assert_eq!(forbidden.status().as_u16(), 403);

    let listed = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .header("x-prodex-sso-role", "viewer")
        .header("x-prodex-sso-key-prefixes", "team-a-")
        .send()
        .expect("SSO viewer list key request should be sent");
    assert_eq!(listed.status().as_u16(), 200);
    let listed: serde_json::Value = listed.json().expect("SSO list response should be json");
    assert_eq!(listed["keys"][0]["name"], "team-a-sso");
}

#[test]
fn gateway_scim_users_can_provision_sso_admin_scope() {
    let root = temp_root("gateway-scim-sso-admin");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-token";
    let sso_token = "sso-proxy-token";
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
        gateway_sso: RuntimeGatewaySsoConfig {
            proxy_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                sso_token,
            )),
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            oidc: None,
            default_role: RuntimeGatewayAdminRole::Viewer,
        },
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

    let created_user = client
        .post(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "userName": "alice@example.com",
            "displayName": "Alice Example",
            "active": true,
            "urn:prodex:params:scim:schemas:gateway:2.0:User": {
                "role": "admin",
                "allowed_key_prefixes": ["team-a-"]
            }
        }))
        .send()
        .expect("SCIM user create request should be sent");
    assert_eq!(created_user.status().as_u16(), 201);
    let created_user: serde_json::Value = created_user
        .json()
        .expect("SCIM create response should be json");
    let user_id = created_user["id"]
        .as_str()
        .expect("SCIM user id should be present")
        .to_string();
    assert_eq!(created_user["userName"], "alice@example.com");

    let listed_users = client
        .get(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("SCIM list request should be sent");
    assert_eq!(listed_users.status().as_u16(), 200);
    let listed_users: serde_json::Value = listed_users
        .json()
        .expect("SCIM list response should be json");
    assert_eq!(listed_users["totalResults"], 1);

    let created_key = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .json(&serde_json::json!({"name": "team-a-scim"}))
        .send()
        .expect("SCIM-backed SSO create key request should be sent");
    assert_eq!(created_key.status().as_u16(), 201);

    let forbidden_key = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .json(&serde_json::json!({"name": "team-b-scim"}))
        .send()
        .expect("SCIM-backed SSO forbidden key request should be sent");
    assert_eq!(forbidden_key.status().as_u16(), 403);

    let deactivated = client
        .patch(format!(
            "http://{}/v1/prodex/gateway/scim/v2/Users/{}",
            proxy.listen_addr, user_id
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "Operations": [
                {"op": "replace", "path": "active", "value": false}
            ]
        }))
        .send()
        .expect("SCIM deactivate request should be sent");
    assert_eq!(deactivated.status().as_u16(), 200);
    let deactivated: serde_json::Value = deactivated
        .json()
        .expect("SCIM deactivate response should be json");
    assert_eq!(deactivated["active"], false);

    let inactive_rejected = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .header("x-prodex-sso-token", sso_token)
        .header("x-prodex-sso-user", "alice@example.com")
        .send()
        .expect("inactive SCIM SSO request should be sent");
    assert_eq!(inactive_rejected.status().as_u16(), 401);
}

#[test]
fn gateway_oidc_jwt_can_authenticate_scoped_admin() {
    let root = temp_root("gateway-oidc-admin");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let jwks = TestJwksServer::start();
    let issuer = "https://idp.example";
    let audience = "prodex-gateway";
    let token =
        gateway_oidc_test_token(issuer, audience, "alice@example.com", "admin", &["team-a-"]);
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
        gateway_sso: RuntimeGatewaySsoConfig {
            proxy_token_hash: None,
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            oidc: Some(RuntimeGatewayOidcConfig {
                issuer: issuer.to_string(),
                audience: audience.to_string(),
                jwks_url: Some(format!("http://{}/jwks.json", jwks.addr)),
                user_claim: "email".to_string(),
                role_claim: "prodex_role".to_string(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: "prodex_key_prefixes".to_string(),
            }),
            default_role: RuntimeGatewayAdminRole::Viewer,
        },
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
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-a-oidc"}))
        .send()
        .expect("OIDC admin create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);

    let forbidden = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-b-oidc"}))
        .send()
        .expect("OIDC admin forbidden key request should be sent");
    assert_eq!(forbidden.status().as_u16(), 403);

    let listed = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(&token)
        .send()
        .expect("OIDC admin list key request should be sent");
    assert_eq!(listed.status().as_u16(), 200);
    let listed: serde_json::Value = listed.json().expect("OIDC list response should be json");
    assert_eq!(listed["keys"][0]["name"], "team-a-oidc");
}

#[test]
fn gateway_oidc_jwt_can_discover_jwks_uri() {
    let root = temp_root("gateway-oidc-discovery-admin");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let oidc = TestOidcDiscoveryServer::start();
    let issuer = format!("http://{}", oidc.addr);
    let audience = "prodex-gateway";
    let token = gateway_oidc_test_token(
        &issuer,
        audience,
        "alice@example.com",
        "admin",
        &["team-a-"],
    );
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
        gateway_sso: RuntimeGatewaySsoConfig {
            proxy_token_hash: None,
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            oidc: Some(RuntimeGatewayOidcConfig {
                issuer: issuer.clone(),
                audience: audience.to_string(),
                jwks_url: None,
                user_claim: "email".to_string(),
                role_claim: "prodex_role".to_string(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: "prodex_key_prefixes".to_string(),
            }),
            default_role: RuntimeGatewayAdminRole::Viewer,
        },
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
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "team-a-discovered-oidc"}))
        .send()
        .expect("OIDC discovery admin create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);
}

#[test]
fn gateway_usage_delta_store_merges_batches_without_losing_counts() {
    let root = temp_root("gateway-usage-delta-merge");
    let path = root.join("gateway-virtual-key-usage.json");
    let ledger_path = root.join("gateway-billing-ledger.jsonl");
    let state_store = RuntimeGatewayStateStore::File {
        key_store_path: root.join("gateway-virtual-keys.json"),
        usage_path: path.clone(),
        ledger_path: ledger_path.clone(),
    };

    runtime_gateway_virtual_key_usage_apply_deltas(
        &state_store,
        &[RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 1,
            key_name: "team-a".to_string(),
            model: "gpt-5.4".to_string(),
            minute_epoch: 100,
            input_tokens: 7,
            estimated_cost_microusd: Some(11),
            created_at_epoch: 1_700_000_000,
        }],
    )
    .expect("first delta batch should save");
    runtime_gateway_virtual_key_usage_apply_deltas(
        &state_store,
        &[RuntimeGatewayVirtualKeyUsageDelta {
            request_id: 2,
            key_name: "team-a".to_string(),
            model: "gpt-5.4".to_string(),
            minute_epoch: 100,
            input_tokens: 13,
            estimated_cost_microusd: Some(17),
            created_at_epoch: 1_700_000_001,
        }],
    )
    .expect("second delta batch should merge");

    let usage = wait_for_json_file(&path);
    assert_eq!(usage["team-a"]["requests_total"], 2);
    assert_eq!(usage["team-a"]["requests_this_minute"], 2);
    assert_eq!(usage["team-a"]["tokens_this_minute"], 20);
    assert_eq!(usage["team-a"]["spend_microusd"], 28);
    let ledger = fs::read_to_string(&ledger_path).expect("ledger should be written");
    assert_eq!(ledger.lines().count(), 2);
    assert!(ledger.contains("\"call_id\":\"prodex-1\""));
    assert!(ledger.contains("\"estimated_cost_microusd\":17"));
}

#[test]
fn gateway_sqlite_state_store_persists_admin_keys_and_usage() {
    let root = temp_root("gateway-sqlite-state");
    let paths = app_paths_for_root(root.clone());
    let db_path = root.join("gateway-state.sqlite");
    let state_store = RuntimeGatewayStateStore::sqlite(db_path.clone());
    let upstream = TestUpstream::start();
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
        gateway_state_store: state_store.clone(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("sqlite gateway proxy should start");
    let client = reqwest::blocking::Client::new();
    let created = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"name": "team-sqlite"}))
        .send()
        .expect("sqlite create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);
    let created: serde_json::Value = created.json().expect("create response should be json");
    let token = created["token"]
        .as_str()
        .expect("generated sqlite token should be returned")
        .to_string();

    let response = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(&token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "sqlite"}))
        .send()
        .expect("sqlite virtual key request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let _ = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive sqlite request");
    wait_for_sqlite_usage_total(&db_path, "team-sqlite", 1);
    wait_for_sqlite_ledger_response_status(&db_path, 2, 200);
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
        gateway_state_store: state_store,
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("sqlite gateway proxy should restart");
    let keys = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            restarted.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("sqlite key list request should be sent");
    assert_eq!(keys.status().as_u16(), 200);
    let keys: serde_json::Value = keys.json().expect("key list response should be json");
    assert_eq!(keys["state_backend"], "sqlite");
    assert_eq!(keys["keys"][0]["name"], "team-sqlite");
    assert_eq!(keys["keys"][0]["usage"]["requests_total"], 1);
    let ledger = client
        .get(format!(
            "http://{}/v1/prodex/gateway/ledger",
            restarted.listen_addr
        ))
        .bearer_auth(admin_token)
        .send()
        .expect("sqlite ledger request should be sent");
    assert_eq!(ledger.status().as_u16(), 200);
    let ledger: serde_json::Value = ledger.json().expect("ledger response should be json");
    assert_eq!(ledger["state_backend"], "sqlite");
    assert_eq!(ledger["records"][0]["key_name"], "team-sqlite");
    assert_eq!(ledger["records"][0]["call_id"], "prodex-2");
    assert_eq!(ledger["records"][0]["response_status"], 200);
    assert_eq!(ledger["records"][0]["output_tokens"], 11);
    assert!(db_path.exists());
    assert!(!root.join("gateway-virtual-keys.json").exists());
}

struct TestUpstream {
    addr: SocketAddr,
    body_rx: mpsc::Receiver<Vec<u8>>,
    _thread: thread::JoinHandle<()>,
}

impl TestUpstream {
    fn start() -> Self {
        Self::start_n(1)
    }

    fn start_n(request_count: usize) -> Self {
        let server = TinyServer::http("127.0.0.1:0").expect("test upstream should bind");
        let addr = server
            .server_addr()
            .to_ip()
            .expect("test upstream should expose TCP addr");
        let (body_tx, body_rx) = mpsc::channel();
        let thread = thread::spawn(move || {
            for _ in 0..request_count {
                let mut request = server.recv().expect("test upstream should receive request");
                let mut body = Vec::new();
                request
                    .as_reader()
                    .read_to_end(&mut body)
                    .expect("test upstream should read request body");
                let _ = body_tx.send(body);
                let mut response = TinyResponse::from_string(
                    r#"{"id":"resp_test","usage":{"input_tokens":7,"output_tokens":11,"total_tokens":18}}"#,
                )
                .with_status_code(200);
                response.add_header(
                    TinyHeader::from_bytes("content-type", "application/json").unwrap(),
                );
                let _ = request.respond(response);
            }
        });
        Self {
            addr,
            body_rx,
            _thread: thread,
        }
    }
}

struct TestJwksServer {
    addr: SocketAddr,
    _thread: thread::JoinHandle<()>,
}

impl TestJwksServer {
    fn start() -> Self {
        let server = TinyServer::http("127.0.0.1:0").expect("test JWKS server should bind");
        let addr = server
            .server_addr()
            .to_ip()
            .expect("test JWKS server should expose TCP addr");
        let thread = thread::spawn(move || {
            for _ in 0..16 {
                let Ok(request) = server.recv() else {
                    break;
                };
                let mut response =
                    TinyResponse::from_string(gateway_oidc_test_jwks()).with_status_code(200);
                response.add_header(
                    TinyHeader::from_bytes("content-type", "application/json").unwrap(),
                );
                let _ = request.respond(response);
            }
        });
        Self {
            addr,
            _thread: thread,
        }
    }
}

struct TestOidcDiscoveryServer {
    addr: SocketAddr,
    _thread: thread::JoinHandle<()>,
}

impl TestOidcDiscoveryServer {
    fn start() -> Self {
        let server = TinyServer::http("127.0.0.1:0").expect("test OIDC server should bind");
        let addr = server
            .server_addr()
            .to_ip()
            .expect("test OIDC server should expose TCP addr");
        let thread = thread::spawn(move || {
            for _ in 0..16 {
                let Ok(request) = server.recv() else {
                    break;
                };
                let path = request.url().to_string();
                let body = if path == "/.well-known/openid-configuration" {
                    format!(r#"{{"jwks_uri":"http://{addr}/jwks.json"}}"#)
                } else {
                    gateway_oidc_test_jwks().to_string()
                };
                let mut response = TinyResponse::from_string(body).with_status_code(200);
                response.add_header(
                    TinyHeader::from_bytes("content-type", "application/json").unwrap(),
                );
                let _ = request.respond(response);
            }
        });
        Self {
            addr,
            _thread: thread,
        }
    }
}

fn gateway_oidc_test_jwks() -> &'static str {
    r#"{"keys":[{"kty":"RSA","n":"yRE6rHuNR0QbHO3H3Kt2pOKGVhQqGZXInOduQNxXzuKlvQTLUTv4l4sggh5_CYYi_cvI-SXVT9kPWSKXxJXBXd_4LkvcPuUakBoAkfh-eiFVMh2VrUyWyj3MFl0HTVF9KwRXLAcwkREiS3npThHRyIxuy0ZMeZfxVL5arMhw1SRELB8HoGfG_AtH89BIE9jDBHZ9dLelK9a184zAf8LwoPLxvJb3Il5nncqPcSfKDDodMFBIMc4lQzDKL5gvmiXLXB1AGLm8KBjfE8s3L5xqi-yUod-j8MtvIj812dkS4QMiRVN_by2h3ZY8LYVGrqZXZTcgn2ujn8uKjXLZVD5TdQ","e":"AQAB","kid":"rsa01","alg":"RS256","use":"sig"}]}"#
}

fn gateway_oidc_test_token(
    issuer: &str,
    audience: &str,
    email: &str,
    role: &str,
    prefixes: &[&str],
) -> String {
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    let claims = serde_json::json!({
        "iss": issuer,
        "aud": audience,
        "sub": email,
        "email": email,
        "prodex_role": role,
        "prodex_key_prefixes": prefixes,
        "exp": exp,
    });
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("rsa01".to_string());
    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(GATEWAY_OIDC_TEST_PRIVATE_KEY.as_bytes())
            .expect("test RSA key should parse"),
    )
    .expect("test OIDC token should sign")
}

const GATEWAY_OIDC_TEST_PRIVATE_KEY: &str = concat!(
    "-----BEGIN ",
    "PRIVATE KEY-----\n",
    r#"MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDJETqse41HRBsc
7cfcq3ak4oZWFCoZlcic525A3FfO4qW9BMtRO/iXiyCCHn8JhiL9y8j5JdVP2Q9Z
IpfElcFd3/guS9w+5RqQGgCR+H56IVUyHZWtTJbKPcwWXQdNUX0rBFcsBzCRESJL
eelOEdHIjG7LRkx5l/FUvlqsyHDVJEQsHwegZ8b8C0fz0EgT2MMEdn10t6Ur1rXz
jMB/wvCg8vG8lvciXmedyo9xJ8oMOh0wUEgxziVDMMovmC+aJctcHUAYubwoGN8T
yzcvnGqL7JSh36Pwy28iPzXZ2RLhAyJFU39vLaHdljwthUaupldlNyCfa6Ofy4qN
ctlUPlN1AgMBAAECggEAdESTQjQ70O8QIp1ZSkCYXeZjuhj081CK7jhhp/4ChK7J
GlFQZMwiBze7d6K84TwAtfQGZhQ7km25E1kOm+3hIDCoKdVSKch/oL54f/BK6sKl
qlIzQEAenho4DuKCm3I4yAw9gEc0DV70DuMTR0LEpYyXcNJY3KNBOTjN5EYQAR9s
2MeurpgK2MdJlIuZaIbzSGd+diiz2E6vkmcufJLtmYUT/k/ddWvEtz+1DnO6bRHh
xuuDMeJA/lGB/EYloSLtdyCF6sII6C6slJJtgfb0bPy7l8VtL5iDyz46IKyzdyzW
tKAn394dm7MYR1RlUBEfqFUyNK7C+pVMVoTwCC2V4QKBgQD64syfiQ2oeUlLYDm4
CcKSP3RnES02bcTyEDFSuGyyS1jldI4A8GXHJ/lG5EYgiYa1RUivge4lJrlNfjyf
dV230xgKms7+JiXqag1FI+3mqjAgg4mYiNjaao8N8O3/PD59wMPeWYImsWXNyeHS
55rUKiHERtCcvdzKl4u35ZtTqQKBgQDNKnX2bVqOJ4WSqCgHRhOm386ugPHfy+8j
m6cicmUR46ND6ggBB03bCnEG9OtGisxTo/TuYVRu3WP4KjoJs2LD5fwdwJqpgtHl
yVsk45Y1Hfo+7M6lAuR8rzCi6kHHNb0HyBmZjysHWZsn79ZM+sQnLpgaYgQGRbKV
DZWlbw7g7QKBgQCl1u+98UGXAP1jFutwbPsx40IVszP4y5ypCe0gqgon3UiY/G+1
zTLp79GGe/SjI2VpQ7AlW7TI2A0bXXvDSDi3/5Dfya9ULnFXv9yfvH1QwWToySpW
Kvd1gYSoiX84/WCtjZOr0e0HmLIb0vw0hqZA4szJSqoxQgvF22EfIWaIaQKBgQCf
34+OmMYw8fEvSCPxDxVvOwW2i7pvV14hFEDYIeZKW2W1HWBhVMzBfFB5SE8yaCQy
pRfOzj9aKOCm2FjjiErVNpkQoi6jGtLvScnhZAt/lr2TXTrl8OwVkPrIaN0bG/AS
aUYxmBPCpXu3UjhfQiWqFq/mFyzlqlgvuCc9g95HPQKBgAscKP8mLxdKwOgX8yFW
GcZ0izY/30012ajdHY+/QK5lsMoxTnn0skdS+spLxaS5ZEO4qvPVb8RAoCkWMMal
2pOhmquJQVDPDLuZHdrIiKiDM20dy9sMfHygWcZjQ4WSxf/J7T9canLZIXFhHAZT
3wc9h4G8BBCtWN2TN/LsGZdB
"#,
    "-----END ",
    "PRIVATE KEY-----",
);

fn temp_root(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("prodex-{name}-{nonce}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn app_paths_for_root(root: std::path::PathBuf) -> AppPaths {
    AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    }
}

fn wait_for_usage_file(path: &std::path::Path) -> serde_json::Value {
    wait_for_json_file(path)
}

fn wait_for_sqlite_usage_total(path: &std::path::Path, key_name: &str, expected: u64) {
    for _ in 0..50 {
        if let Ok(conn) = rusqlite::Connection::open(path) {
            let total = conn
                .query_row(
                    "SELECT requests_total FROM prodex_gateway_virtual_key_usage WHERE key_name = ?1",
                    [key_name],
                    |row| row.get::<_, i64>(0),
                )
                .ok()
                .and_then(|value| u64::try_from(value).ok());
            if total == Some(expected) {
                return;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "sqlite usage total for {key_name} did not reach {expected} at {}",
        path.display()
    );
}

fn wait_for_ledger_file_response_status(path: &std::path::Path, request: u64, expected: u16) {
    for _ in 0..50 {
        if let Ok(bytes) = fs::read(path) {
            for line in String::from_utf8_lossy(&bytes).lines() {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
                    && value["request"] == request
                    && value["response_status"] == expected
                {
                    return;
                }
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "ledger response status for request {request} did not reach {expected} at {}",
        path.display()
    );
}

fn wait_for_ledger_file_key_response_status(path: &std::path::Path, key_name: &str, expected: u16) {
    for _ in 0..50 {
        if let Ok(bytes) = fs::read(path) {
            for line in String::from_utf8_lossy(&bytes).lines() {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
                    && value["key_name"] == key_name
                    && value["response_status"] == expected
                {
                    return;
                }
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "ledger response status for key {key_name} did not reach {expected} at {}",
        path.display()
    );
}

fn wait_for_sqlite_ledger_response_status(path: &std::path::Path, request: u64, expected: u16) {
    for _ in 0..50 {
        if let Ok(conn) = rusqlite::Connection::open(path) {
            let status = conn
                .query_row(
                    "SELECT response_status FROM prodex_gateway_billing_ledger WHERE request_id = ?1",
                    [i64::try_from(request).unwrap_or(i64::MAX)],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .ok()
                .flatten()
                .and_then(|value| u16::try_from(value).ok());
            if status == Some(expected) {
                return;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "sqlite ledger response status for request {request} did not reach {expected} at {}",
        path.display()
    );
}

fn wait_for_json_file(path: &std::path::Path) -> serde_json::Value {
    for _ in 0..50 {
        if let Ok(bytes) = fs::read(path)
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            return value;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("usage file was not written at {}", path.display());
}
