use super::*;

#[test]
fn translate_runtime_anthropic_messages_request_maps_tools_and_tool_results() {
    let _effort_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_REASONING_EFFORT");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![
            ("User-Agent".to_string(), "claude-cli/test".to_string()),
            (
                "X-Claude-Code-Session-Id".to_string(),
                "claude-session-123".to_string(),
            ),
        ],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 1024,
            "stream": true,
            "thinking": {
                "type": "adaptive"
            },
            "system": [
                {
                    "type": "text",
                    "text": "System instructions",
                }
            ],
            "output_config": {
                "effort": "medium",
            },
            "tools": [
                {
                    "name": "shell",
                    "description": "Run a shell command",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "tool_choice": {
                "type": "any"
            },
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "Show the logs",
                        },
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": "YWJj",
                            }
                        }
                    ]
                },
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "text",
                            "text": "Calling shell",
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_1",
                            "name": "shell",
                            "input": {
                                "cmd": "ls"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_1",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "file1",
                                }
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    assert_eq!(
        translated.translated_request.path_and_query,
        "/backend-api/codex/responses"
    );
    assert_eq!(translated.requested_model, "claude-sonnet-4-6");
    assert!(translated.stream);
    assert!(translated.want_thinking);
    assert_eq!(
        runtime_proxy_request_header_value(&translated.translated_request.headers, "session_id"),
        Some("claude-session-123")
    );

    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("model").and_then(serde_json::Value::as_str),
        Some("gpt-5.3-codex")
    );
    assert_eq!(
        body.get("instructions").and_then(serde_json::Value::as_str),
        Some("System instructions")
    );
    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );
    assert!(
        body.get("max_tokens").is_none(),
        "translated request should not send unsupported max_tokens"
    );
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("medium")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("parameters"))
            .and_then(|parameters| parameters.get("properties"))
            .and_then(serde_json::Value::as_object)
            .map(serde_json::Map::len),
        Some(0)
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(input.len(), 4);
    assert_eq!(
        input[0].get("role").and_then(serde_json::Value::as_str),
        Some("user")
    );
    assert_eq!(
        input[0]
            .get("content")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        input[1].get("role").and_then(serde_json::Value::as_str),
        Some("assistant")
    );
    assert_eq!(
        input[1].get("content").and_then(serde_json::Value::as_str),
        Some("Calling shell")
    );
    assert_eq!(
        input[2].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[2].get("call_id").and_then(serde_json::Value::as_str),
        Some("toolu_1")
    );
    assert_eq!(
        input[3].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );
    assert_eq!(
        input[3].get("output").and_then(serde_json::Value::as_str),
        Some("file1")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_tool_references() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_search",
                            "name": "ToolSearch",
                            "input": {
                                "query": "select:WebSearch,WebFetch,TodoWrite"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_search",
                            "content": [
                                {
                                    "type": "tool_reference",
                                    "tool_name": "WebSearch",
                                },
                                {
                                    "type": "tool_reference",
                                    "tool_name": "WebFetch",
                                },
                                {
                                    "type": "tool_reference",
                                    "tool_name": "TodoWrite",
                                }
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 2);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("ToolSearch")
    );
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );

    let output = input[1]
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("tool result output should be a string");
    let output: serde_json::Value =
        serde_json::from_str(output).expect("tool result output should be valid JSON");
    assert_eq!(
        output
            .get("tool_references")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            }),
        Some(vec!["WebSearch", "WebFetch", "TodoWrite"])
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_keeps_upstream_streaming_for_non_stream_client_request()
 {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "stream": false,
            "messages": [
                {
                    "role": "user",
                    "content": "Cari halaman utama openai.com"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    assert!(
        !translated.stream,
        "Anthropic client request should remain buffered locally"
    );

    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("stream").and_then(serde_json::Value::as_bool),
        Some(true),
        "Responses upstream must stay streaming for server-tool follow-up compatibility"
    );
}

#[test]
fn runtime_request_for_anthropic_server_tool_followup_defaults_stream_to_true() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.3-codex",
            "input": [
                {
                    "role": "user",
                    "content": "Cari halaman utama openai.com"
                }
            ],
            "tool_choice": "auto"
        })
        .to_string()
        .into_bytes(),
    };

    let followup =
        runtime_request_for_anthropic_server_tool_followup(&request, "resp_followup_123")
            .expect("follow-up request should serialize");
    let body: serde_json::Value =
        serde_json::from_slice(&followup.body).expect("follow-up body should parse");

    assert_eq!(
        body.get("previous_response_id")
            .and_then(serde_json::Value::as_str),
        Some("resp_followup_123")
    );
    assert!(
        body.get("input").is_none(),
        "follow-up request should omit original input"
    );
    assert!(
        body.get("tool_choice").is_none(),
        "follow-up request should omit tool choice"
    );
    assert_eq!(
        body.get("stream").and_then(serde_json::Value::as_bool),
        Some(true),
        "follow-up request must request a streaming Responses transport"
    );
}

