use super::{
    AppState, RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
    TestUpstream, app_paths_for_root, start_runtime_local_rewrite_proxy, temp_root,
};
use serde_json::{Value, json};
use std::fs;
use std::time::Duration;

const CHAT: &str = r#"{"id":"chatcmpl_local","object":"chat.completion","model":"offrail-27b","choices":[{"index":0,"message":{"role":"assistant","content":"LOCAL_OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":4,"completion_tokens":2,"total_tokens":6}}"#;

fn local_proxy(
    paths: &crate::AppPaths,
    upstream: &TestUpstream,
    local: bool,
) -> crate::RuntimeRotationProxy {
    start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/prefix/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: Vec::new(),
            local,
        },
        upstream_no_proxy: true,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(118272),
        preferred_listen_addr: Some("127.0.0.1:0"),
    })
    .unwrap()
}

#[test]
fn local_chat_transport_preserves_model_effort_tools_history_and_endpoint_prefix() {
    let paths = app_paths_for_root(temp_root("local-chat-roundtrip"));
    let upstream = TestUpstream::start_with_responses(vec![200], "application/json", CHAT);
    let proxy = local_proxy(&paths, &upstream, true);
    let body = json!({"model":"offrail-27b","stream":false,"reasoning":{"effort":"medium"},
        "input":[{"role":"system","content":"primary rules"},{"role":"user","content":"read a fixture"},{"role":"developer","content":"extra rules"},
            {"type":"function_call","call_id":"call1","name":"read_file","namespace":"mcp__fixture","arguments":"{\"path\":\"fixture.txt\"}"},
            {"type":"function_call_output","call_id":"call1","output":"fixture content"}],
        "tools":[{"type":"custom","name":"apply_patch","description":"Edit fixture files.","format":{"type":"text"}},
            {"type":"namespace","name":"mcp__fixture","tools":[{"type":"function","name":"read_file","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}}]}]});
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .header("authorization", "Bearer synthetic-proxy-auth")
        .json(&body)
        .send()
        .unwrap();
    let status = response.status();
    let text = response.text().unwrap();
    assert_eq!(status.as_u16(), 200, "{text}");
    let result: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(result["object"], "response", "{result}");
    assert!(
        result["output"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["content"].to_string().contains("LOCAL_OK")),
        "{result}"
    );
    assert_eq!(
        upstream
            .path_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
        "/prefix/v1/chat/completions"
    );
    let sent: Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(sent["model"], "offrail-27b");
    assert_eq!(
        sent["messages"][0],
        json!({"role":"system","content":"primary rules\n\nextra rules"})
    );
    assert!(
        sent["messages"]
            .as_array()
            .unwrap()
            .iter()
            .skip(1)
            .all(|message| message["role"] != "system")
    );
    assert_eq!(sent["reasoning_effort"], "medium");
    let tools = sent["tools"].as_array().unwrap();
    assert!(tools.iter().all(|tool| tool["type"] == "function"));
    assert_eq!(tools[0]["function"]["name"], "apply_patch");
    assert_eq!(
        tools[0]["function"]["parameters"]["properties"]["input"]["type"],
        "string"
    );
    assert_eq!(tools[1]["function"]["name"], "mcp__fixture__read_file");
    assert!(
        sent["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["role"] == "tool" && item["content"] == "fixture content")
    );
    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(!headers.iter().any(|(key, _)| key == "authorization"));
    drop(proxy);
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn local_chat_alias_does_not_change_cloud_or_generic_responses_routing() {
    for (model, local) in [("offrail-27b", false), ("generic-local", true)] {
        let paths = app_paths_for_root(temp_root("local-chat-isolation"));
        let upstream = TestUpstream::start();
        let proxy = local_proxy(&paths, &upstream, local);
        let response = reqwest::blocking::Client::new()
            .post(format!("http://{}/v1/responses", proxy.listen_addr))
            .json(&json!({"model":model,"input":"fixture","stream":false}))
            .send()
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(
            upstream
                .path_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            "/prefix/v1/responses"
        );
        let sent: Value = serde_json::from_slice(
            &upstream
                .body_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(sent["input"], "fixture");
        assert!(sent.get("messages").is_none());
        drop(response);
        drop(proxy);
        fs::remove_dir_all(paths.root).unwrap();
    }
}

#[test]
fn local_chat_upstream_validation_errors_keep_their_status_without_retry() {
    let paths = app_paths_for_root(temp_root("local-chat-validation"));
    let upstream = TestUpstream::start_with_responses(
        vec![400],
        "application/json",
        r#"{"error":{"type":"invalid_request_error","message":"unsupported fixture"}}"#,
    );
    let proxy = local_proxy(&paths, &upstream, true);
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&json!({"model":"offrail-27b","input":"fixture","stream":false}))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 400);
    assert!(response.text().unwrap().contains("unsupported fixture"));
    drop(proxy);
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn openai_compatible_transport_preserves_trace_context_for_chat_completions() {
    let root = temp_root("openai-compatible-trace-context");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            local: false,
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
    })
    .expect("OpenAI-compatible local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .header(reqwest::header::USER_AGENT, "codex-cli/0.1-test")
        .header("authorization", "Bearer caller-token")
        .header(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .header("tracestate", "prodex=test")
        .header("baggage", "tenant_tier=premium")
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "stream": false,
            "messages": [{"role": "user", "content": "trace me"}]
        }))
        .send()
        .expect("OpenAI-compatible chat request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive OpenAI-compatible headers");
    let header = |name: &str| {
        headers
            .iter()
            .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value.as_str()))
    };
    assert_eq!(header("authorization"), Some("Bearer upstream-key"));
    assert_eq!(header("user-agent"), Some("codex-cli/0.1-test"));
    assert_eq!(
        header("traceparent"),
        Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
    );
    assert_eq!(header("tracestate"), Some("prodex=test"));
    assert_eq!(header("baggage"), Some("tenant_tier=premium"));
}

#[test]
fn unsupported_local_effort_is_a_client_error_before_upstream_contact() {
    let paths = app_paths_for_root(temp_root("local-effort-validation"));
    // No server responses are available: any upstream attempt would fail this contract.
    let upstream = TestUpstream::start_with_responses(Vec::new(), "application/json", CHAT);
    let proxy = local_proxy(&paths, &upstream, true);
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&json!({"model":"offrail-27b","input":"fixture","reasoning":{"effort":"max"}}))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 400);
    let value: Value = response.json().unwrap();
    assert_eq!(value["error"]["code"], "unsupported_reasoning_effort");
    assert!(upstream.body_rx.try_recv().is_err());
    drop(proxy);
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn local_chat_compaction_accepts_history_without_new_tool_definitions() {
    let body = json!({"model":"offrail-27b","stream":true,"tools":[],"tool_choice":"auto",
        "parallel_tool_calls":false,"reasoning":{"effort":"medium"},"store":false,
        "include":["reasoning.encrypted_content"],"input":[
        {"type":"message","role":"developer","content":[{"type":"input_text","text":"Follow instructions"}]},
        {"type":"message","role":"user","content":[{"type":"input_text","text":"write a file"}]},
        {"type":"custom_tool_call","id":"ctc_fixture","call_id":"call_fixture","name":"apply_patch","input":"*** Begin Patch\n*** Add File: fixture.txt\n+hello\n*** End Patch"},
        {"type":"custom_tool_call_output","id":"ctco_fixture","call_id":"call_fixture","output":"Success"},
        {"type":"message","role":"assistant","content":[{"type":"output_text","text":"Done"}]},
        {"type":"message","role":"user","content":[{"type":"input_text","text":"Summarize our progress"}]}]});
    let paths = app_paths_for_root(temp_root("local-compact-control"));
    let upstream = TestUpstream::start_with_responses(vec![200], "application/json", CHAT);
    let proxy = local_proxy(&paths, &upstream, true);
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&body)
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let chat: Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(chat["parallel_tool_calls"], false);
    assert!(
        chat["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["role"] == "tool" && message["content"] == "Success")
    );
    drop(response);
    drop(proxy);
    fs::remove_dir_all(paths.root).unwrap();
}

#[path = "local_resume.rs"]
mod local_resume;

#[path = "local_auth.rs"]
mod local_auth;
