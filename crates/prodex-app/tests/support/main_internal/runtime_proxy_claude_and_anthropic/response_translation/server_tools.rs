use super::*;

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
        RuntimeAnthropicServerToolUsage {
            web_search_requests: translated.carried_web_search_requests,
            web_fetch_requests: translated.carried_web_fetch_requests,
            code_execution_requests: translated.carried_code_execution_requests,
            tool_search_requests: translated.carried_tool_search_requests,
        },
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
        RuntimeAnthropicServerToolUsage {
            web_search_requests: translated.carried_web_search_requests,
            web_fetch_requests: translated.carried_web_fetch_requests,
            code_execution_requests: translated.carried_code_execution_requests,
            tool_search_requests: translated.carried_tool_search_requests,
        },
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
        RuntimeAnthropicServerToolUsage {
            web_search_requests: translated.carried_web_search_requests,
            web_fetch_requests: translated.carried_web_fetch_requests,
            code_execution_requests: translated.carried_code_execution_requests,
            tool_search_requests: translated.carried_tool_search_requests,
        },
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
        RuntimeAnthropicServerToolUsage {
            web_search_requests: translated.carried_web_search_requests,
            web_fetch_requests: translated.carried_web_fetch_requests,
            code_execution_requests: translated.carried_code_execution_requests,
            tool_search_requests: translated.carried_tool_search_requests,
        },
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
