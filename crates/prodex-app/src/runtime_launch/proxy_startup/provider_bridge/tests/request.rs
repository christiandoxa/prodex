use super::super::{RuntimeProviderBridgeKind, runtime_provider_request_conformance_result};
use prodex_provider_core::ProviderTransformLoss;

#[test]
fn request_conformance_result_uses_provider_core_translator_for_deepseek_and_gemini() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: vec![
            ("x-codex-turn-state".to_string(), "turn-1".to_string()),
            ("session_id".to_string(), "sess-1".to_string()),
        ],
        body: Vec::new(),
    };
    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        br#"{"model":"deepseek-chat","input":"hello"}"#,
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    assert_eq!(
        deepseek.metadata["continuation"]["x-codex-turn-state"],
        "turn-1"
    );

    let gemini = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        &request,
        br#"{"model":"gemini-2.5-pro","input":"hello","response_format":{"type":"json_schema","schema":{"type":"object","strict":true}}}"#,
    )
    .unwrap();
    assert!(matches!(gemini.loss, ProviderTransformLoss::Lossless));
    assert_eq!(
        gemini.metadata["continuation"]["x-codex-turn-state"],
        "turn-1"
    );
    assert_eq!(gemini.metadata["continuation"]["session_id"], "sess-1");
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        body["request"]["generationConfig"]["responseMimeType"],
        "application/json"
    );
}

#[test]
fn request_conformance_result_treats_compact_route_as_responses_translation_surface() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses/compact".to_string(),
        headers: vec![
            ("x-codex-turn-state".to_string(), "turn-compact".to_string()),
            ("session_id".to_string(), "sess-compact".to_string()),
        ],
        body: Vec::new(),
    };

    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        br#"{"model":"deepseek-chat","input":"hello","previous_response_id":"resp_compact"}"#,
    )
    .expect("deepseek compact conformance");
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    assert_eq!(
        deepseek.metadata["continuation"]["session_id"],
        "sess-compact"
    );
    assert_eq!(
        deepseek.metadata["continuation"]["previous_response_id"],
        "resp_compact"
    );

    let gemini = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        &request,
        br#"{"model":"gemini-2.5-pro","input":"hello","previous_response_id":"resp_compact"}"#,
    )
    .expect("gemini compact conformance");
    assert!(matches!(gemini.loss, ProviderTransformLoss::Lossless));
    assert_eq!(
        gemini.metadata["continuation"]["session_id"],
        "sess-compact"
    );
    assert_eq!(
        gemini.metadata["continuation"]["previous_response_id"],
        "resp_compact"
    );
}

#[test]
fn gemini_request_conformance_translates_message_content_arrays() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let gemini = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "# AGENTS.md instructions for /repo"},
                        {"type": "input_text", "text": "<environment_context><cwd>/repo</cwd></environment_context>"}
                    ]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "Run the update"}
                    ]
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(gemini.loss, ProviderTransformLoss::Lossless));
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        body["request"]["contents"][0]["parts"][0]["text"],
        "Run the update"
    );
    assert!(
        body["request"]["systemInstruction"]["parts"][0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("# AGENTS.md instructions for /repo"))
    );
}

#[test]
fn deepseek_request_conformance_translates_message_content_arrays() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "system",
                    "content": [{"type": "input_text", "text": "You are Codex."}]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "read commit history"}]
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    let body: serde_json::Value = serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "You are Codex.");
    assert_eq!(body["messages"][1]["content"], "read commit history");
}

#[test]
fn deepseek_request_conformance_preserves_assistant_tool_history() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "find the symbol"}]
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": ""}],
                    "tool_calls": [
                        {
                            "id":"call_1",
                            "type":"function",
                            "function":{
                                "name":"grep",
                                "arguments":"{\"pattern\":\"ProviderTranslator\"}"
                            }
                        }
                    ]
                },
                {
                    "type": "message",
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": [{"type": "output_text", "text": "{\"match_count\":1}"}]
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    let body: serde_json::Value = serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["messages"][1]["role"], "assistant");
    assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call_1");
    assert_eq!(
        body["messages"][1]["tool_calls"][0]["function"]["name"],
        "grep"
    );
    assert_eq!(body["messages"][2]["role"], "tool");
    assert_eq!(body["messages"][2]["tool_call_id"], "call_1");
}

#[test]
fn deepseek_request_conformance_translates_function_call_and_output_items() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "find the symbol"}]
                },
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "grep",
                    "arguments": {"pattern": "ProviderTranslator"}
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "{\"match_count\":1}"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    let body: serde_json::Value = serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["messages"][1]["role"], "assistant");
    assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call_1");
    assert_eq!(
        body["messages"][1]["tool_calls"][0]["function"]["arguments"],
        "{\"pattern\":\"ProviderTranslator\"}"
    );
    assert_eq!(body["messages"][2]["role"], "tool");
    assert_eq!(body["messages"][2]["tool_call_id"], "call_1");
}

#[test]
fn deepseek_request_conformance_translates_custom_and_shell_call_items() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let custom = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "custom_tool_call",
                    "call_id": "call_patch_1",
                    "name": "apply_patch",
                    "input": "*** Begin Patch\n*** End Patch"
                },
                {
                    "type": "custom_tool_call_output",
                    "call_id": "call_patch_1",
                    "output": "applied"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let custom_body: serde_json::Value =
        serde_json::from_slice(custom.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        custom_body["messages"][0]["tool_calls"][0]["function"]["name"],
        "apply_patch"
    );
    assert_eq!(
        custom_body["messages"][0]["tool_calls"][0]["function"]["arguments"],
        "{\"input\":\"*** Begin Patch\\n*** End Patch\"}"
    );
    assert_eq!(custom_body["messages"][1]["tool_call_id"], "call_patch_1");

    let shell = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "local_shell_call",
                    "call_id": "call_shell_1",
                    "action": {
                        "command": ["git", "status", "--short"]
                    },
                    "cwd": "/repo",
                    "timeout": 1000
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_shell_1",
                    "output": " M Cargo.toml"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let shell_body: serde_json::Value =
        serde_json::from_slice(shell.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        shell_body["messages"][0]["tool_calls"][0]["function"]["name"],
        "shell_command"
    );
    assert_eq!(
        shell_body["messages"][0]["tool_calls"][0]["function"]["arguments"],
        "{\"command\":\"git status --short\",\"cwd\":\"/repo\",\"timeout\":1000}"
    );
    assert_eq!(shell_body["messages"][1]["tool_call_id"], "call_shell_1");
}

#[test]
fn deepseek_request_conformance_translates_mcp_call_and_result_items() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "compress this text"}]
                },
                {
                    "type": "mcp_call",
                    "id": "call_sqz_1",
                    "name": "mcp__prodex_sqz__compress",
                    "arguments": {"text": "large repeated content"},
                    "output": "ref:abc123"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    let body: serde_json::Value = serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        body["messages"][1]["tool_calls"][0]["function"]["name"],
        "mcp__prodex_sqz__compress"
    );
    assert_eq!(
        body["messages"][1]["tool_calls"][0]["function"]["arguments"],
        "{\"text\":\"large repeated content\"}"
    );
    assert_eq!(body["messages"][2]["tool_call_id"], "call_sqz_1");
    assert_eq!(body["messages"][2]["content"], "ref:abc123");
}

#[test]
fn request_conformance_result_skips_non_responses_paths() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/chat/completions".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    assert!(
        runtime_provider_request_conformance_result(
            RuntimeProviderBridgeKind::DeepSeek,
            &request,
            br#"{"model":"deepseek-chat","input":"hello"}"#,
        )
        .is_none()
    );
}
