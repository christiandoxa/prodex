#[test]
fn runtime_anthropic_response_from_json_value_preserves_versioned_client_tools_as_generic_tool_use()
{
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 11,
                "output_tokens": 5
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_bash",
                    "name": "bash_20250124",
                    "arguments": "{\"command\":\"ls -la\"}"
                },
                {
                    "type": "function_call",
                    "call_id": "call_text_editor",
                    "name": "text_editor_20250124",
                    "arguments": "{\"path\":\"/tmp/prodex/notes.txt\"}"
                },
                {
                    "type": "function_call",
                    "call_id": "call_computer",
                    "name": "computer_20250124",
                    "arguments": "{\"display_width_px\":1440}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");

    assert_eq!(
        content
            .iter()
            .map(|block| block.get("type").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![Some("tool_use"), Some("tool_use"), Some("tool_use")]
    );
    assert_eq!(
        content
            .iter()
            .map(|block| block.get("name").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![
            Some("bash_20250124"),
            Some("text_editor_20250124"),
            Some("computer_20250124"),
        ]
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_maps_shell_call_to_bash_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 11,
                "output_tokens": 5
            },
            "output": [
                {
                    "type": "shell_call",
                    "call_id": "call_shell",
                    "action": {
                        "commands": ["ls -la"],
                        "timeout_ms": 1200,
                        "max_output_length": 4096
                    }
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[0].get("name").and_then(serde_json::Value::as_str),
        Some("bash")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("ls -la")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("timeout_ms"))
            .and_then(serde_json::Value::as_u64),
        Some(1200)
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("max_output_length"))
            .and_then(serde_json::Value::as_u64),
        Some(4096)
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_joins_shell_call_commands_for_bash_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 9,
                "output_tokens": 4
            },
            "output": [
                {
                    "type": "shell_call",
                    "call_id": "call_shell_multi",
                    "action": {
                        "commands": ["pwd", "ls -la"]
                    }
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("pwd\nls -la")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_maps_computer_call_to_computer_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            },
            "output": [
                {
                    "type": "computer_call",
                    "call_id": "call_computer",
                    "actions": [
                        {
                            "type": "click",
                            "button": "left",
                            "x": 321,
                            "y": 654
                        }
                    ]
                }
            ]
        }),
        "claude-opus-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[0].get("name").and_then(serde_json::Value::as_str),
        Some("computer")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("action"))
            .and_then(serde_json::Value::as_str),
        Some("left_click")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("coordinate"))
            .and_then(serde_json::Value::as_array)
            .map(|coordinates| {
                coordinates
                    .iter()
                    .filter_map(serde_json::Value::as_i64)
                    .collect::<Vec<_>>()
            }),
        Some(vec![321, 654])
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_raw_computer_actions_when_not_lossless() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 8,
                "output_tokens": 4
            },
            "output": [
                {
                    "type": "computer_call",
                    "call_id": "call_computer_raw",
                    "actions": [
                        {
                            "type": "screenshot"
                        },
                        {
                            "type": "click",
                            "button": "left",
                            "x": 10,
                            "y": 20
                        }
                    ]
                }
            ]
        }),
        "claude-opus-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("actions"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_memory_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 7,
                "output_tokens": 3
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_memory",
                    "name": "memory",
                    "arguments": "{\"command\":\"view\",\"path\":\"/memories\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[0].get("name").and_then(serde_json::Value::as_str),
        Some("memory")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("view")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_tool_use_and_usage() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7,
                "input_tokens_details": {
                    "cached_tokens": 3
                }
            },
            "output": [
                {
                    "type": "reasoning",
                    "summary": [
                        {
                            "type": "summary_text",
                            "text": "Plan first",
                        }
                    ]
                },
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Hello from prodex",
                        }
                    ]
                },
                {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "shell",
                    "arguments": "{\"cmd\":\"ls\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        true,
    );

    assert_eq!(
        response.get("type").and_then(serde_json::Value::as_str),
        Some("message")
    );
    assert_eq!(
        response.get("model").and_then(serde_json::Value::as_str),
        Some("claude-sonnet-4-6")
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("thinking")
    );
    assert_eq!(
        content[1].get("text").and_then(serde_json::Value::as_str),
        Some("Hello from prodex")
    );
    assert_eq!(
        content[2].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[2].get("id").and_then(serde_json::Value::as_str),
        Some("call_123")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("cache_read_input_tokens"))
            .and_then(serde_json::Value::as_u64),
        Some(3)
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_counts_web_search_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "WebSearch",
                    "arguments": "{\"query\":\"OpenAI latest news today\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_counts_web_fetch_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_456",
                    "name": "web_fetch",
                    "arguments": "{\"url\":\"https://example.com/article\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_counts_tool_search_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_search",
                    "name": "tool_search_tool_regex",
                    "arguments": "{\"query\":\"browser_fetch\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("tool_search_tool_regex")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("tool_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_uses_registered_server_tool_aliases() {
    let mut server_tools = RuntimeAnthropicServerTools::default();
    server_tools.register("browser_fetch", "web_fetch");
    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_alias",
                    "name": "browser_fetch",
                    "arguments": "{\"url\":\"https://example.com/article\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
        0,
        0,
        0,
        0,
        Some(&server_tools),
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_claude_web_fetch_tool_use() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "WebFetch"
            },
            "tools": [
                {
                    "name": "WebFetch",
                    "description": "Fetch a web page.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "url": {
                                "type": "string"
                            },
                            "prompt": {
                                "type": "string"
                            }
                        },
                        "required": ["url"]
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Lihat isi https://example.com"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };
    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");

    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_webfetch",
                    "name": "WebFetch",
                    "arguments": "{\"url\":\"https://example.com\",\"prompt\":\"Summarize the page\"}"
                }
            ]
        }),
        &translated.requested_model,
        translated.want_thinking,
        translated.carried_web_search_requests,
        translated.carried_web_fetch_requests,
        translated.carried_code_execution_requests,
        translated.carried_tool_search_requests,
        Some(&translated.server_tools),
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebFetch")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("url"))
            .and_then(serde_json::Value::as_str),
        Some("https://example.com")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_claude_web_search_tool_use() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "WebSearch"
            },
            "tools": [
                {
                    "name": "WebSearch",
                    "description": "Search the web for current information.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string"
                            },
                            "allowed_domains": {
                                "type": "array",
                                "items": {
                                    "type": "string"
                                }
                            }
                        },
                        "required": ["query"]
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Cari berita terbaru"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };
    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");

    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_websearch",
                    "name": "WebSearch",
                    "arguments": "{\"query\":\"OpenAI latest news today\",\"allowed_domains\":[\"openai.com\"]}"
                }
            ]
        }),
        &translated.requested_model,
        translated.want_thinking,
        translated.carried_web_search_requests,
        translated.carried_web_fetch_requests,
        translated.carried_code_execution_requests,
        translated.carried_tool_search_requests,
        Some(&translated.server_tools),
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("query"))
            .and_then(serde_json::Value::as_str),
        Some("OpenAI latest news today")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_generic_server_tool_use_from_request_state()
{
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
                            "type": "server_tool_use",
                            "id": "srvtoolu_browser",
                            "name": "browser_fetch",
                            "input": {
                                "url": "https://example.com/article"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": "done"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };
    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");

    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "srvtoolu_browser",
                    "name": "browser_fetch",
                    "arguments": "{\"url\":\"https://example.com/article\"}"
                }
            ]
        }),
        &translated.requested_model,
        translated.want_thinking,
        translated.carried_web_search_requests,
        translated.carried_web_fetch_requests,
        translated.carried_code_execution_requests,
        translated.carried_tool_search_requests,
        Some(&translated.server_tools),
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("browser_fetch")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_code_execution_server_tool_use() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tools": [
                {
                    "type": "code_execution_20250825"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Analyze the attached data"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };
    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");

    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "srvtoolu_code",
                    "name": "code_execution",
                    "arguments": "{\"code\":\"print(1 + 1)\"}"
                }
            ]
        }),
        &translated.requested_model,
        translated.want_thinking,
        translated.carried_web_search_requests,
        translated.carried_web_fetch_requests,
        translated.carried_code_execution_requests,
        translated.carried_tool_search_requests,
        Some(&translated.server_tools),
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("code_execution")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_maps_mcp_call_to_blocks() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "mcp_call",
                    "id": "mcp_1",
                    "name": "read_file",
                    "server_label": "local_fs",
                    "arguments": "{\"path\":\"/tmp/prodex/AGENTS.md\"}",
                    "output": "AGENTS content"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("mcp_tool_use")
    );
    assert_eq!(
        content[0]
            .get("server_name")
            .and_then(serde_json::Value::as_str),
        Some("local_fs")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("path"))
            .and_then(serde_json::Value::as_str),
        Some("/tmp/prodex/AGENTS.md")
    );
    assert_eq!(
        content[1].get("type").and_then(serde_json::Value::as_str),
        Some("mcp_tool_result")
    );
    assert_eq!(
        content[1]
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("text"))
            .and_then(serde_json::Value::as_str),
        Some("AGENTS content")
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("end_turn")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_mcp_approval_request_and_list_tools() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 7,
                "output_tokens": 3
            },
            "output": [
                {
                    "type": "mcp_approval_request",
                    "id": "mcpapr_1",
                    "name": "read_file",
                    "server_label": "local_fs",
                    "arguments": "{\"path\":\"/tmp/prodex/AGENTS.md\"}"
                },
                {
                    "type": "mcp_list_tools",
                    "id": "mcplist_1",
                    "server_label": "local_fs",
                    "tools": [
                        {
                            "name": "read_file",
                            "description": "Read a file",
                            "input_schema": {
                                "type": "object",
                                "properties": {
                                    "path": {
                                        "type": "string"
                                    }
                                }
                            }
                        }
                    ]
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("mcp_approval_request")
    );
    assert_eq!(
        content[0].get("id").and_then(serde_json::Value::as_str),
        Some("mcpapr_1")
    );
    assert_eq!(
        content[0]
            .get("server_label")
            .and_then(serde_json::Value::as_str),
        Some("local_fs")
    );
    assert_eq!(
        content[1].get("type").and_then(serde_json::Value::as_str),
        Some("mcp_list_tools")
    );
    assert_eq!(
        content[1]
            .get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("read_file")
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
}

#[test]
fn runtime_anthropic_sse_response_parts_from_message_value_preserves_mcp_approval_and_list_tools() {
    let message = serde_json::json!({
        "id": "msg_mcp_adv",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "mcp_approval_request",
                "id": "mcpapr_2",
                "name": "delete_file",
                "server_label": "local_fs",
                "arguments": "{\"path\":\"/tmp/prodex/old.txt\"}"
            },
            {
                "type": "mcp_list_tools",
                "id": "mcplist_2",
                "server_label": "local_fs",
                "tools": [
                    {
                        "name": "delete_file",
                        "description": "Delete a file",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string"
                                }
                            }
                        }
                    }
                ]
            }
        ],
        "model": "claude-sonnet-4-6",
        "stop_reason": "end_turn",
        "stop_sequence": serde_json::Value::Null,
        "usage": {
            "input_tokens": 4,
            "output_tokens": 2,
            "server_tool_use": {
                "web_search_requests": 0,
                "web_fetch_requests": 0
            }
        }
    });

    let parts = runtime_anthropic_sse_response_parts_from_message_value(message);
    let body = String::from_utf8(parts.body.into_vec()).expect("SSE body should decode");

    assert!(body.contains("\"mcp_approval_request\""));
    assert!(body.contains("\"mcp_list_tools\""));
    assert!(body.contains("\"mcpapr_2\""));
    assert!(body.contains("\"mcplist_2\""));
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_web_search_results() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "web_search_call",
                    "id": "ws_123",
                    "status": "completed",
                    "action": {
                        "type": "search",
                        "queries": ["berita teknologi terbaru"],
                        "sources": [
                            {
                                "type": "url",
                                "url": "https://example.com/story"
                            }
                        ]
                    }
                },
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Ringkasan singkat.",
                            "annotations": [
                                {
                                    "type": "url_citation",
                                    "url": "https://example.com/story",
                                    "title": "Example Story"
                                }
                            ]
                        }
                    ]
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("query"))
            .and_then(serde_json::Value::as_str),
        Some("berita teknologi terbaru")
    );
    assert_eq!(
        content[1].get("type").and_then(serde_json::Value::as_str),
        Some("web_search_tool_result")
    );
    assert_eq!(
        content[1]
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|results| results.first())
            .and_then(|result| result.get("url"))
            .and_then(serde_json::Value::as_str),
        Some("https://example.com/story")
    );
    assert_eq!(
        content[1]
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|results| results.first())
            .and_then(|result| result.get("title"))
            .and_then(serde_json::Value::as_str),
        Some("Example Story")
    );
    assert_eq!(
        content[2].get("type").and_then(serde_json::Value::as_str),
        Some("text")
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("end_turn")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_collects_deltas_and_tool_use() {
    let body = concat!(
        "event: response.reasoning_summary_text.delta\r\n",
        "data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"Plan.\"}\r\n",
        "\r\n",
        "event: response.output_text.delta\r\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\r\n",
        "\r\n",
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"shell\"}}\r\n",
        "\r\n",
        "event: response.function_call_arguments.delta\r\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"call_id\":\"call_1\",\"delta\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"shell\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4}}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", true)
        .expect("SSE translation should succeed");
    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("thinking")
    );
    assert_eq!(
        content[1].get("text").and_then(serde_json::Value::as_str),
        Some("Hello")
    );
    assert_eq!(
        content[2].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("input_tokens"))
            .and_then(serde_json::Value::as_u64),
        Some(9)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_maps_shell_call_to_bash_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"shell_call\",\"call_id\":\"call_shell\"}}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"shell_call\",\"call_id\":\"call_shell\",\"action\":{\"commands\":[\"pwd\",\"ls -la\"],\"timeout_ms\":1200}}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":7,\"output_tokens\":3},\"output\":[{\"type\":\"shell_call\",\"call_id\":\"call_shell\",\"action\":{\"commands\":[\"pwd\",\"ls -la\"],\"timeout_ms\":1200}}]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();

    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("bash")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("pwd\nls -la")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("timeout_ms"))
            .and_then(serde_json::Value::as_u64),
        Some(1200)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_maps_computer_call_to_computer_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"computer_call\",\"call_id\":\"call_computer\"}}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"computer_call\",\"call_id\":\"call_computer\",\"actions\":[{\"type\":\"move\",\"x\":100,\"y\":200}]}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":6,\"output_tokens\":3},\"output\":[{\"type\":\"computer_call\",\"call_id\":\"call_computer\",\"actions\":[{\"type\":\"move\",\"x\":100,\"y\":200}]}]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();

    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-opus-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("computer")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("action"))
            .and_then(serde_json::Value::as_str),
        Some("mouse_move")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("coordinate"))
            .and_then(serde_json::Value::as_array)
            .map(|coordinates| {
                coordinates
                    .iter()
                    .filter_map(serde_json::Value::as_i64)
                    .collect::<Vec<_>>()
            }),
        Some(vec![100, 200])
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_preserves_web_search_usage() {
    let body = concat!(
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_123\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"queries\":[\"OpenAI latest news today\"],\"sources\":[{\"type\":\"url\",\"url\":\"https://openai.com/index/industrial-policy-for-the-intelligence-age\",\"title\":\"Industrial policy for the Intelligence Age\"}]}}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"tool_usage\":{\"web_search\":{\"num_requests\":1}},\"output\":[{\"type\":\"web_search_call\",\"id\":\"ws_123\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"queries\":[\"OpenAI latest news today\"],\"sources\":[{\"type\":\"url\",\"url\":\"https://openai.com/index/industrial-policy-for-the-intelligence-age\",\"title\":\"Industrial policy for the Intelligence Age\"}]}},{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"OpenAI published a new industrial policy post.\"}]}]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");
    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        content[1].get("type").and_then(serde_json::Value::as_str),
        Some("web_search_tool_result")
    );
    assert_eq!(
        content[2].get("text").and_then(serde_json::Value::as_str),
        Some("OpenAI published a new industrial policy post.")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_counts_web_search_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"WebSearch\"}}\r\n",
        "\r\n",
        "event: response.function_call_arguments.delta\r\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"call_id\":\"call_1\",\"delta\":\"{\\\"query\\\":\\\"OpenAI latest news today\\\"}\"}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"WebSearch\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"output\":[]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_counts_web_fetch_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_2\",\"name\":\"web_fetch\"}}\r\n",
        "\r\n",
        "event: response.function_call_arguments.delta\r\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"call_id\":\"call_2\",\"delta\":\"{\\\"url\\\":\\\"https://example.com/article\\\"}\"}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_2\",\"name\":\"web_fetch\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"output\":[]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_proxy_anthropic_carried_server_tool_usage_counts_latest_tool_chain_suffix() {
    let messages = serde_json::json!([
        {
            "role": "user",
            "content": "older prompt"
        },
        {
            "role": "assistant",
            "content": [
                {
                    "type": "server_tool_use",
                    "id": "srvtoolu_old",
                    "name": "web_search",
                    "input": {
                        "query": "older query"
                    }
                }
            ]
        },
        {
            "role": "user",
            "content": [
                {
                    "type": "web_search_tool_result",
                    "tool_use_id": "srvtoolu_old",
                    "content": []
                }
            ]
        },
        {
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "Older answer"
                }
            ]
        },
        {
            "role": "assistant",
            "content": [
                {
                    "type": "server_tool_use",
                    "id": "srvtoolu_latest",
                    "name": "web_search",
                    "input": {
                        "query": "latest query"
                    }
                }
            ]
        },
        {
            "role": "user",
            "content": [
                {
                    "type": "web_search_tool_result",
                    "tool_use_id": "srvtoolu_latest",
                    "content": []
                }
            ]
        }
    ]);

    let usage = runtime_proxy_anthropic_carried_server_tool_usage(
        messages.as_array().expect("messages should be an array"),
    );

    assert_eq!(usage.web_search_requests, 1);
    assert_eq!(usage.web_fetch_requests, 0);
    assert_eq!(usage.tool_search_requests, 0);
}

#[test]
fn runtime_anthropic_response_from_json_value_applies_carried_web_search_usage() {
    let value = serde_json::json!({
        "usage": {
            "input_tokens": 9,
            "output_tokens": 4
        },
        "output": [
            {
                "type": "message",
                "content": [
                    {
                        "type": "output_text",
                        "text": "Done."
                    }
                ]
            }
        ]
    });

    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &value,
        "claude-sonnet-4-6",
        false,
        1,
        0,
        0,
        0,
        None,
    );

    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_sse_response_parts_from_message_value_seeds_server_tool_use_in_message_start()
{
    let message = serde_json::json!({
        "id": "msg_123",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "text",
                "text": "OpenAI posted a new update."
            }
        ],
        "model": "claude-sonnet-4-6",
        "stop_reason": "end_turn",
        "stop_sequence": serde_json::Value::Null,
        "usage": {
            "input_tokens": 9,
            "output_tokens": 4,
            "server_tool_use": {
                "web_search_requests": 1,
                "web_fetch_requests": 0
            }
        }
    });

    let parts = runtime_anthropic_sse_response_parts_from_message_value(message);
    let body = String::from_utf8(parts.body.into_vec()).expect("SSE body should decode");

    assert!(body.contains("\"server_tool_use\""));
    assert!(body.contains("\"web_search_requests\":1"));
    assert!(body.contains("\"web_fetch_requests\":0"));
}

#[test]
fn runtime_anthropic_sse_response_parts_from_message_value_preserves_web_fetch_tool_result() {
    let message = serde_json::json!({
        "id": "msg_fetch",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "server_tool_use",
                "id": "srvtoolu_fetch",
                "name": "web_fetch",
                "input": {
                    "url": "https://example.com/article"
                }
            },
            {
                "type": "web_fetch_tool_result",
                "tool_use_id": "srvtoolu_fetch",
                "content": {
                    "type": "web_fetch_result",
                    "url": "https://example.com/article"
                }
            }
        ],
        "model": "claude-sonnet-4-6",
        "stop_reason": "end_turn",
        "stop_sequence": serde_json::Value::Null,
        "usage": {
            "input_tokens": 9,
            "output_tokens": 4,
            "server_tool_use": {
                "web_search_requests": 0,
                "web_fetch_requests": 1
            }
        }
    });

    let parts = runtime_anthropic_sse_response_parts_from_message_value(message);
    let body = String::from_utf8(parts.body.into_vec()).expect("SSE body should decode");

    assert!(body.contains("\"web_fetch_tool_result\""));
    assert!(body.contains("https://example.com/article"));
}

#[test]
fn runtime_anthropic_sse_response_parts_from_message_value_preserves_mcp_server_name() {
    let message = serde_json::json!({
        "id": "msg_mcp",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "mcp_tool_use",
                "id": "mcp_1",
                "name": "read_file",
                "server_name": "local_fs",
                "input": {
                    "path": "/tmp/prodex/AGENTS.md"
                }
            },
            {
                "type": "mcp_tool_result",
                "tool_use_id": "mcp_1",
                "content": [
                    {
                        "type": "text",
                        "text": "AGENTS body"
                    }
                ]
            }
        ],
        "model": "claude-sonnet-4-6",
        "stop_reason": "end_turn",
        "stop_sequence": serde_json::Value::Null,
        "usage": {
            "input_tokens": 4,
            "output_tokens": 2,
            "server_tool_use": {
                "web_search_requests": 0,
                "web_fetch_requests": 0
            }
        }
    });

    let parts = runtime_anthropic_sse_response_parts_from_message_value(message);
    let body = String::from_utf8(parts.body.into_vec()).expect("SSE body should decode");

    assert!(body.contains("\"mcp_tool_use\""));
    assert!(body.contains("\"server_name\":\"local_fs\""));
    assert!(body.contains("\"mcp_tool_result\""));
}

#[test]
fn runtime_anthropic_sse_response_parts_from_message_value_preserves_generic_tool_result_blocks() {
    let message = serde_json::json!({
        "id": "msg_456",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "bash_code_execution_tool_result",
                "tool_use_id": "srvtoolu_code",
                "content": {
                    "type": "bash_code_execution_result",
                    "stdout": "ok",
                    "stderr": "",
                    "return_code": 0
                }
            }
        ],
        "model": "claude-sonnet-4-6",
        "stop_reason": "end_turn",
        "stop_sequence": serde_json::Value::Null,
        "usage": {
            "input_tokens": 3,
            "output_tokens": 2,
            "server_tool_use": {
                "web_search_requests": 0,
                "web_fetch_requests": 0
            }
        }
    });

    let parts = runtime_anthropic_sse_response_parts_from_message_value(message);
    let body = String::from_utf8(parts.body.into_vec()).expect("sse body should be valid utf-8");
    assert!(body.contains("\"bash_code_execution_tool_result\""));
    assert!(body.contains("\"srvtoolu_code\""));
    assert!(body.contains("\"stdout\":\"ok\""));
}

#[test]
fn runtime_anthropic_sse_response_parts_from_responses_sse_bytes_preserves_carried_server_tool_usage(
) {
    let body = concat!(
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"output\":[]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();

    let parts = runtime_anthropic_sse_response_parts_from_responses_sse_bytes(
        &body,
        "claude-sonnet-4-6",
        false,
        1,
        1,
        0,
        1,
        &RuntimeAnthropicServerTools::default(),
    )
    .expect("buffered SSE translation should succeed");
    let body = String::from_utf8(parts.body.into_vec()).expect("SSE body should decode");

    assert!(body.contains("\"web_search_requests\":1"));
    assert!(body.contains("\"web_fetch_requests\":1"));
    assert!(body.contains("\"tool_search_requests\":1"));
}

#[test]
fn runtime_anthropic_sse_reader_emits_web_search_usage_in_message_delta() {
    let upstream = concat!(
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"WebSearch\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"tool_usage\":{\"web_search\":{\"num_requests\":1}},\"output\":[]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let mut reader = RuntimeAnthropicSseReader::new(
        Box::new(Cursor::new(upstream)),
        "claude-sonnet-4-6".to_string(),
        false,
        0,
        0,
        0,
        0,
        RuntimeAnthropicServerTools::default(),
    );
    let mut emitted = Vec::new();
    reader
        .read_to_end(&mut emitted)
        .expect("translated SSE stream should read");
    let body = String::from_utf8(emitted).expect("translated SSE body should decode");

    assert!(body.contains("event: message_delta"));
    assert!(body.contains("\"server_tool_use\""));
    assert!(body.contains("\"web_search_requests\":1"));
    assert!(body.contains("\"web_fetch_requests\":0"));
}

#[test]
fn runtime_anthropic_sse_reader_preserves_mcp_tool_result_error_flag() {
    let upstream = concat!(
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"mcp_call\",\"id\":\"mcp_1\",\"name\":\"delete_file\",\"server_label\":\"local_fs\",\"arguments\":\"{\\\"path\\\":\\\"/tmp/prodex/old.txt\\\"}\",\"error\":\"permission denied\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"output\":[{\"type\":\"mcp_call\",\"id\":\"mcp_1\",\"name\":\"delete_file\",\"server_label\":\"local_fs\",\"arguments\":\"{\\\"path\\\":\\\"/tmp/prodex/old.txt\\\"}\",\"error\":\"permission denied\"}]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let mut reader = RuntimeAnthropicSseReader::new(
        Box::new(Cursor::new(upstream)),
        "claude-sonnet-4-6".to_string(),
        false,
        0,
        0,
        0,
        0,
        RuntimeAnthropicServerTools::default(),
    );
    let mut emitted = Vec::new();
    reader
        .read_to_end(&mut emitted)
        .expect("translated SSE stream should read");
    let body = String::from_utf8(emitted).expect("translated SSE body should decode");

    assert!(body.contains("\"mcp_tool_result\""));
    assert!(body.contains("\"permission denied\""));
    assert!(body.contains("\"is_error\":true"));
}
