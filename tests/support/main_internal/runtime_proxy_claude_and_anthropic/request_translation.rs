#[test]
fn translate_runtime_anthropic_messages_request_maps_tools_and_tool_results() {
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
fn translate_runtime_anthropic_messages_request_keeps_upstream_streaming_for_non_stream_client_request(
) {
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

#[test]
fn translate_runtime_anthropic_messages_request_maps_document_content_to_input_text() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "document",
                            "source": {
                                "type": "text",
                                "media_type": "text/plain",
                                "data": "Document body",
                            }
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

    assert_eq!(input.len(), 1);
    assert_eq!(
        input[0].get("role").and_then(serde_json::Value::as_str),
        Some("user")
    );
    assert_eq!(
        input[0].get("content").and_then(serde_json::Value::as_str),
        Some("Document body")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_web_fetch_tool_result() {
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
                            "id": "srvtoolu_fetch",
                            "name": "web_fetch",
                            "input": {
                                "url": "https://example.com/article"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "web_fetch_tool_result",
                            "tool_use_id": "srvtoolu_fetch",
                            "content": {
                                "type": "web_fetch_result",
                                "url": "https://example.com/article",
                                "content": {
                                    "type": "document",
                                    "source": {
                                        "type": "text",
                                        "media_type": "text/plain",
                                        "data": "Example content",
                                    },
                                    "title": "Example Article",
                                },
                                "retrieved_at": "2026-04-09T03:00:00Z",
                            }
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
        Some("web_fetch")
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
        output.get("type").and_then(serde_json::Value::as_str),
        Some("web_fetch_result")
    );
    assert_eq!(
        output.get("url").and_then(serde_json::Value::as_str),
        Some("https://example.com/article")
    );
    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some("Example content")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_code_execution_tool_result() {
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
                            "id": "srvtoolu_code",
                            "name": "bash_code_execution",
                            "input": {
                                "command": "ls -la | head -5"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "bash_code_execution_tool_result",
                            "tool_use_id": "srvtoolu_code",
                            "content": {
                                "type": "bash_code_execution_result",
                                "stdout": "file_a\nfile_b",
                                "stderr": "warning: ignored file",
                                "return_code": 0
                            }
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
        Some("bash_code_execution")
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
        output.get("type").and_then(serde_json::Value::as_str),
        Some("bash_code_execution_result")
    );
    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some("file_a\nfile_b\nwarning: ignored file")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_file_backed_document_and_container_blocks(
) {
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
                            "id": "srvtoolu_file",
                            "name": "web_fetch",
                            "input": {
                                "url": "https://example.com/file"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "srvtoolu_file",
                            "content": [
                                {
                                    "type": "document",
                                    "source": {
                                        "type": "base64",
                                        "media_type": "text/plain",
                                        "data": "RmlsZSBib2R5"
                                    },
                                    "title": "File payload"
                                },
                                {
                                    "type": "container_upload",
                                    "container_id": "container_123",
                                    "path": "/tmp/prodex/input.txt"
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

    let output = input
        .iter()
        .find(|item| {
            item.get("type").and_then(serde_json::Value::as_str) == Some("function_call_output")
        })
        .expect("function_call_output should exist");
    let output: serde_json::Value = serde_json::from_str(
        output
            .get("output")
            .and_then(serde_json::Value::as_str)
            .expect("output should be a string"),
    )
    .expect("output should parse");

    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some("File body")
    );
    let content_blocks = output
        .get("content_blocks")
        .and_then(serde_json::Value::as_array)
        .expect("content_blocks should be preserved");
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("document")
            && block
                .get("source")
                .and_then(|source| source.get("type"))
                .and_then(serde_json::Value::as_str)
                == Some("base64")
    }));
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("container_upload")
            && block
                .get("container_id")
                .and_then(serde_json::Value::as_str)
                == Some("container_123")
    }));
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_generic_server_tool_use_name() {
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
        input[0].get("call_id").and_then(serde_json::Value::as_str),
        Some("srvtoolu_browser")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("browser_fetch")
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            input[0]
                .get("arguments")
                .and_then(serde_json::Value::as_str)
                .expect("arguments should be a string")
        )
        .expect("arguments should parse"),
        serde_json::json!({
            "url": "https://example.com/article"
        })
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_mcp_tool_use_server_name() {
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
                            "type": "mcp_tool_use",
                            "id": "mcpu_1",
                            "name": "filesystem",
                            "server_name": "local_fs",
                            "input": {
                                "path": "/tmp/prodex"
                            }
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

    assert_eq!(input.len(), 1);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("call_id").and_then(serde_json::Value::as_str),
        Some("mcpu_1")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("filesystem")
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            input[0]
                .get("arguments")
                .and_then(serde_json::Value::as_str)
                .expect("arguments should be a string")
        )
        .expect("arguments should parse"),
        serde_json::json!({
            "path": "/tmp/prodex",
            "server_name": "local_fs"
        })
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_accepts_mcp_servers_with_mcp_tool_use() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "mcp_servers": [
                {
                    "name": "filesystem",
                    "type": "stdio",
                    "command": "mcp-fs"
                }
            ],
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "mcp_tool_use",
                            "id": "mcpu_2",
                            "name": "filesystem",
                            "server_name": "local_fs",
                            "input": {
                                "path": "/tmp/prodex"
                            }
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

    assert_eq!(input.len(), 1);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("call_id").and_then(serde_json::Value::as_str),
        Some("mcpu_2")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("filesystem")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_mcp_toolset_to_responses_mcp() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "mcp_servers": [
                {
                    "name": "local_fs",
                    "type": "url",
                    "url": "https://mcp.example.com/sse",
                    "authorization_token": "token_123"
                }
            ],
            "tools": [
                {
                    "type": "mcp_toolset",
                    "mcp_server_name": "local_fs",
                    "description": "Local filesystem tools",
                    "default_config": {
                        "enabled": false,
                        "defer_loading": true
                    },
                    "configs": {
                        "read_file": {
                            "enabled": true
                        }
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Read the project file"
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
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("tools array should exist");

    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("mcp")
    );
    assert_eq!(
        tools[0]
            .get("server_label")
            .and_then(serde_json::Value::as_str),
        Some("local_fs")
    );
    assert_eq!(
        tools[0]
            .get("server_url")
            .and_then(serde_json::Value::as_str),
        Some("https://mcp.example.com/sse")
    );
    assert_eq!(
        tools[0]
            .get("authorization")
            .and_then(serde_json::Value::as_str),
        Some("token_123")
    );
    assert_eq!(
        tools[0]
            .get("require_approval")
            .and_then(serde_json::Value::as_str),
        Some("never")
    );
    assert_eq!(
        tools[0]
            .get("defer_loading")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        tools[0].get("allowed_tools"),
        Some(&serde_json::json!(["read_file"]))
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_does_not_buffer_mcp_only_toolsets() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-api-key".to_string(), "dummy".to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "stream": true,
            "mcp_servers": [
                {
                    "name": "local_fs",
                    "url": "https://mcp.example.com/sse"
                }
            ],
            "tools": [
                {
                    "type": "mcp_toolset",
                    "mcp_server_name": "local_fs",
                    "name": "filesystem"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "List the workspace files."
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");

    assert!(
        !translated.server_tools.needs_buffered_translation(),
        "mcp-only anthropic requests should stay on the streaming path"
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_falls_back_for_unrepresentable_mcp_toolset_denylist(
) {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "mcp_servers": [
                {
                    "name": "local_fs",
                    "type": "url",
                    "url": "https://mcp.example.com/sse"
                }
            ],
            "tools": [
                {
                    "type": "mcp_toolset",
                    "mcp_server_name": "local_fs",
                    "configs": {
                        "delete_file": {
                            "enabled": false
                        }
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the workspace"
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
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("tools array should exist");

    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        tools[0].get("name").and_then(serde_json::Value::as_str),
        Some("mcp_toolset")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_unsupported_user_content_blocks() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "document",
                            "source": {
                                "type": "file",
                                "file_id": "file_123",
                                "filename": "report.pdf"
                            }
                        },
                        {
                            "type": "container_upload",
                            "container_id": "container_123",
                            "path": "/tmp/prodex/report.pdf"
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

    assert_eq!(input.len(), 1);
    let content = input[0]
        .get("content")
        .and_then(serde_json::Value::as_str)
        .expect("content should be a string fallback");
    assert!(content.contains("[anthropic:document]"));
    assert!(content.contains("\"file_id\":\"file_123\""));
    assert!(content.contains("[anthropic:container_upload]"));
}

#[test]
fn translate_runtime_anthropic_messages_request_roundtrips_search_result_content_blocks() {
    let search_result = serde_json::json!({
        "type": "search_result",
        "source": "https://example.com/search?q=prodex",
        "title": "Prodex Search Hit",
        "snippet": "Prodex wraps codex and manages isolated profiles."
    });
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
                            "name": "LocalSearch",
                            "input": {
                                "query": "prodex"
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
                                search_result.clone()
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
        Some("LocalSearch")
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

    assert!(
        output.get("text").is_none(),
        "structured-only search results should not be flattened into synthetic text"
    );
    assert_eq!(
        output.get("content_blocks"),
        Some(&serde_json::json!([search_result]))
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_roundtrips_structured_tool_result_content_blocks() {
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
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "srvtoolu_browser",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "Fetched body"
                                },
                                {
                                    "type": "tool_reference",
                                    "tool_name": "browser_fetch"
                                },
                                {
                                    "type": "document",
                                    "source": {
                                        "type": "text",
                                        "media_type": "text/plain",
                                        "data": "Document text"
                                    },
                                    "title": "Doc title"
                                },
                                {
                                    "type": "web_fetch_result",
                                    "url": "https://example.com/article",
                                    "content": {
                                        "type": "document",
                                        "source": {
                                            "type": "text",
                                            "media_type": "text/plain",
                                            "data": "Inner fetch body"
                                        },
                                        "title": "Inner title"
                                    },
                                    "retrieved_at": "2026-04-09T03:00:00Z"
                                },
                                {
                                    "type": "custom_block",
                                    "details": "keep me"
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
        Some("browser_fetch")
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
        Some(vec!["browser_fetch"])
    );
    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some("Fetched body\nDocument text\nInner fetch body")
    );

    let content_blocks = output
        .get("content_blocks")
        .and_then(serde_json::Value::as_array)
        .expect("content_blocks should be preserved");
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("text")
            && block.get("text").and_then(serde_json::Value::as_str) == Some("Fetched body")
    }));
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("tool_reference")
            && block.get("tool_name").and_then(serde_json::Value::as_str) == Some("browser_fetch")
    }));
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("document")
    }));
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("web_fetch_result")
    }));
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("custom_block")
    }));
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_structured_error_tool_result_content() {
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
                            "id": "toolu_text_editor",
                            "name": "text_editor_20250124",
                            "input": {
                                "path": "/tmp/prodex/notes.txt",
                                "old_string": "foo",
                                "new_string": "bar"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_text_editor",
                            "is_error": true,
                            "content": [
                                {
                                    "type": "text",
                                    "text": "Replacement failed."
                                },
                                {
                                    "type": "tool_reference",
                                    "tool_name": "str_replace_based_edit_tool"
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

    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );

    let output = input[1]
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("tool result output should be a string");
    let output: serde_json::Value =
        serde_json::from_str(output).expect("tool result output should remain valid JSON");

    assert_eq!(
        output.get("is_error").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some("Replacement failed.")
    );
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
        Some(vec!["str_replace_based_edit_tool"])
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_keeps_versioned_builtin_client_tools() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "computer_20250124"
            },
            "tools": [
                {
                    "type": "bash_20250124",
                    "description": "Run shell commands"
                },
                {
                    "type": "text_editor_20250124",
                    "description": "Edit local files"
                },
                {
                    "type": "computer_20250124",
                    "display_width_px": 1440,
                    "display_height_px": 900,
                    "display_number": 1
                },
                {
                    "type": "memory_20250818",
                    "description": "Persist project memory"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Open the browser and inspect the page"
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
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");

    assert_eq!(tools.len(), 4);
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.get("type").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![
            Some("function"),
            Some("function"),
            Some("function"),
            Some("function"),
        ]
    );
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.get("name").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![
            Some("bash"),
            Some("str_replace_based_edit_tool"),
            Some("computer"),
            Some("memory"),
        ]
    );
    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Run shell commands")
    );
    assert_eq!(
        tools[1]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Edit local files")
    );
    assert_eq!(
        tools[2]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Interact with the graphical computer display. Display resolution: 1440x900 pixels. Display number: 1."
        )
    );
    assert_eq!(
        tools[3]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Persist project memory")
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("old_str"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert!(tools[1]
        .get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(|properties| properties.get("command"))
        .and_then(|property| property.get("enum"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("undo_edit"))));
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("insert_text"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[2]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("action"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[2]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("coordinate"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("array")
    );
    assert_eq!(
        tools[3]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert!(tools[3]
        .get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(|properties| properties.get("command"))
        .and_then(|property| property.get("enum"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("rename"))));
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("computer")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_appends_computer_display_context_to_existing_description(
) {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tools": [
                {
                    "type": "computer_20250124",
                    "description": "Inspect the browser UI.",
                    "display_width_px": 1280,
                    "display_height_px": 720,
                    "display_number": 2
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the page"
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
    let description = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .and_then(|tools| tools.first())
        .and_then(|tool| tool.get("description"))
        .and_then(serde_json::Value::as_str);

    assert_eq!(
        description,
        Some(
            "Inspect the browser UI.\n\nInteract with the graphical computer display. Display resolution: 1280x720 pixels. Display number: 2."
        )
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_modern_builtin_tool_capabilities() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tools": [
                {
                    "type": "text_editor_20250728",
                    "description": "Edit project files.",
                    "max_characters": 8192
                },
                {
                    "type": "computer_20251124",
                    "description": "Inspect the browser UI.",
                    "display_width_px": 1600,
                    "display_height_px": 900,
                    "display_number": 1,
                    "enable_zoom": true
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the project state"
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
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");

    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Edit project files.\n\nEdit local text files with string replacement operations. View results may be truncated to 8192 characters."
        )
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("enum"))
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            }),
        Some(vec!["view", "create", "str_replace", "insert"])
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("insert_text"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Inspect the browser UI.\n\nInteract with the graphical computer display. Display resolution: 1600x900 pixels. Display number: 1. Zoom action enabled."
        )
    );
    assert!(tools[1]
        .get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(|properties| properties.get("action"))
        .and_then(|property| property.get("enum"))
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
        })
        .is_some_and(|values| values.contains(&"zoom")));
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("region"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("array")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_modern_builtin_client_tool_capabilities()
{
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-opus-4-6",
            "tools": [
                {
                    "type": "text_editor_20250728",
                    "name": "str_replace_based_edit_tool",
                    "max_characters": 10000
                },
                {
                    "type": "computer_20251124",
                    "display_width_px": 1600,
                    "display_height_px": 900,
                    "display_number": 1,
                    "enable_zoom": true
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect and edit the project files."
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
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");

    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Edit local text files with string replacement operations. View results may be truncated to 10000 characters."
        )
    );
    assert!(!tools[0]
        .get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(|properties| properties.get("command"))
        .and_then(|property| property.get("enum"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("undo_edit"))));
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("insert_text"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Interact with the graphical computer display. Display resolution: 1600x900 pixels. Display number: 1. Zoom action enabled."
        )
    );
    assert!(tools[1]
        .get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(|properties| properties.get("action"))
        .and_then(|property| property.get("enum"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("zoom"))));
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("region"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("array")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_bash_tool_to_native_shell_when_enabled() {
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "shell");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "bash_20250124"
            },
            "tools": [
                {
                    "type": "bash_20250124",
                    "description": "Run shell commands"
                }
            ],
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "text",
                            "text": "Running ls."
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_bash",
                            "name": "bash",
                            "input": {
                                "command": "ls -la",
                                "timeout_ms": 1200,
                                "max_output_length": 4096
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_bash",
                            "content": "file1\nfile2"
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
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("shell")
    );
    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(
        input[0].get("role").and_then(serde_json::Value::as_str),
        Some("assistant")
    );
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("shell_call")
    );
    assert_eq!(
        input[1].get("call_id").and_then(serde_json::Value::as_str),
        Some("toolu_bash")
    );
    assert_eq!(
        input[1]
            .get("action")
            .and_then(|action| action.get("commands"))
            .and_then(serde_json::Value::as_array)
            .and_then(|commands| commands.first())
            .and_then(serde_json::Value::as_str),
        Some("ls -la")
    );
    assert_eq!(
        input[2].get("type").and_then(serde_json::Value::as_str),
        Some("shell_call_output")
    );
    assert_eq!(
        input[2]
            .get("max_output_length")
            .and_then(serde_json::Value::as_u64),
        Some(4096)
    );
    assert_eq!(
        input[2]
            .get("output")
            .and_then(serde_json::Value::as_array)
            .and_then(|output| output.first())
            .and_then(|entry| entry.get("stdout"))
            .and_then(serde_json::Value::as_str),
        Some("file1\nfile2")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_computer_tool_to_native_computer_when_enabled()
{
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "computer");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-opus-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "computer_20250124"
            },
            "tools": [
                {
                    "type": "computer_20250124",
                    "display_width_px": 1440,
                    "display_height_px": 900,
                    "display_number": 1
                }
            ],
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "text",
                            "text": "Capturing the current screen."
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_computer",
                            "name": "computer",
                            "input": {
                                "action": "screenshot"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_computer",
                            "content": [
                                {
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": "image/png",
                                        "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO7+o2kAAAAASUVORK5CYII="
                                    }
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

    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("computer")
    );
    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("computer_call")
    );
    assert_eq!(
        input[1]
            .get("actions")
            .and_then(serde_json::Value::as_array)
            .and_then(|actions| actions.first())
            .and_then(|action| action.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("screenshot")
    );
    assert_eq!(
        input[2].get("type").and_then(serde_json::Value::as_str),
        Some("computer_call_output")
    );
    assert_eq!(
        input[2]
            .get("output")
            .and_then(|output| output.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("computer_screenshot")
    );
    assert!(input[2]
        .get("output")
        .and_then(|output| output.get("image_url"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| value.starts_with("data:image/png;base64,")));
}

#[test]
fn translate_runtime_anthropic_messages_request_falls_back_for_ambiguous_native_computer_tool_choice(
) {
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "computer");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-opus-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "computer_20250124"
            },
            "tools": [
                {
                    "type": "computer_20250124",
                    "display_width_px": 1440,
                    "display_height_px": 900
                },
                {
                    "name": "lookup_logs",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the UI"
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

    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .map(|tools| {
                tools
                    .iter()
                    .map(|tool| tool.get("type").and_then(serde_json::Value::as_str))
                    .collect::<Vec<_>>()
            }),
        Some(vec![Some("function"), Some("function")])
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("computer")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_falls_back_for_ambiguous_native_bash_tool_choice() {
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "shell");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "bash_20250124"
            },
            "tools": [
                {
                    "type": "bash_20250124",
                    "description": "Run shell commands"
                },
                {
                    "name": "lookup_logs",
                    "description": "Read application logs",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the logs"
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
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.get("type").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![Some("function"), Some("function")]
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("bash")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_normalizes_versioned_client_tool_choice_aliases() {
    let cases = [
        ("bash_20250124", "bash"),
        ("text_editor_20250124", "str_replace_based_edit_tool"),
        ("computer_20250124", "computer"),
        ("memory_20250818", "memory"),
    ];

    for (versioned_tool_type, expected_name) in cases {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/v1/messages?beta=true".to_string(),
            headers: vec![],
            body: serde_json::json!({
                "model": "claude-sonnet-4-6",
                "tool_choice": {
                    "type": "tool",
                    "name": versioned_tool_type
                },
                "tools": [
                    {
                        "type": versioned_tool_type,
                        "description": "Run a versioned client tool"
                    }
                ],
                "messages": [
                    {
                        "role": "user",
                        "content": "Use the requested tool"
                    }
                ]
            })
            .to_string()
            .into_bytes(),
        };

        let translated = translate_runtime_anthropic_messages_request(&request)
            .expect("translation should succeed");
        let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
            .expect("translated body should parse");

        assert_eq!(
            body.get("tools")
                .and_then(serde_json::Value::as_array)
                .and_then(|tools| tools.first())
                .and_then(|tool| tool.get("type"))
                .and_then(serde_json::Value::as_str),
            Some("function"),
            "tool type should remain a generic function for {versioned_tool_type}"
        );
        assert_eq!(
            body.get("tools")
                .and_then(serde_json::Value::as_array)
                .and_then(|tools| tools.first())
                .and_then(|tool| tool.get("name"))
                .and_then(serde_json::Value::as_str),
            Some(expected_name),
            "tool name should normalize for {versioned_tool_type}"
        );
        assert_eq!(
            body.get("tool_choice")
                .and_then(|tool_choice| tool_choice.get("type"))
                .and_then(serde_json::Value::as_str),
            Some("function"),
            "tool_choice type should normalize for {versioned_tool_type}"
        );
        assert_eq!(
            body.get("tool_choice")
                .and_then(|tool_choice| tool_choice.get("name"))
                .and_then(serde_json::Value::as_str),
            Some(expected_name),
            "tool_choice name should normalize for {versioned_tool_type}"
        );
    }
}

#[test]
fn translate_runtime_anthropic_messages_request_roundtrips_versioned_text_editor_tool_use_and_result(
) {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "text_editor_20250124"
            },
            "tools": [
                {
                    "type": "text_editor_20250124",
                    "description": "Edit local files"
                }
            ],
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_text_editor",
                            "name": "text_editor_20250124",
                            "input": {
                                "path": "/tmp/prodex/notes.txt",
                                "old_string": "foo",
                                "new_string": "bar"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_text_editor",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "Replaced 1 occurrence."
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

    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        tools[0].get("name").and_then(serde_json::Value::as_str),
        Some("str_replace_based_edit_tool")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("str_replace_based_edit_tool")
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("text_editor_20250124")
    );
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );
    assert_eq!(
        input[1].get("output").and_then(serde_json::Value::as_str),
        Some("Replaced 1 occurrence.")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_memory_tool_definition_to_builtin_function() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "memory_20250818"
            },
            "tools": [
                {
                    "type": "memory_20250818"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Remember that I prefer concise answers"
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
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("tools array should exist");

    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        tools[0].get("name").and_then(serde_json::Value::as_str),
        Some("memory")
    );
    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Store and retrieve information across conversations using persistent memory files.")
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|command| command.get("enum"))
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            }),
        Some(vec![
            "view",
            "create",
            "str_replace",
            "insert",
            "delete",
            "rename",
        ])
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("memory")
    );
    assert!(
        body.get("instructions")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|instructions| instructions.contains("/memories")),
        "memory tool should append memory guidance to instructions"
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_appends_memory_tool_guidance_to_system_instructions(
) {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "system": "System instructions",
            "tools": [
                {
                    "type": "memory_20250818",
                    "name": "memory"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Remember that I prefer concise answers"
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
    let instructions = body
        .get("instructions")
        .and_then(serde_json::Value::as_str)
        .expect("instructions should exist");

    assert!(instructions.contains("System instructions"));
    assert!(instructions.contains("/memories"));
    assert!(instructions.contains("memory"));
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_versioned_code_execution_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "code_execution"
            },
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
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("code_execution")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("code_execution")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_versioned_tool_search_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "tool_search_tool_regex"
            },
            "tools": [
                {
                    "type": "tool_search_tool_regex_20251119"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Find tools related to browser fetch"
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
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("tool_search_tool_regex")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("parameters"))
            .and_then(|parameters| parameters.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("object")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("tool_search_tool_regex")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_versioned_web_fetch_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "web_fetch"
            },
            "tools": [
                {
                    "type": "web_fetch_20260409",
                    "description": "Fetch the contents of a web page.",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Fetch https://example.com"
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

    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_compacts_verbose_web_search_tool_result_text() {
    let raw_output = concat!(
        "Web search results for query: \"berita reksadana terbaru Indonesia April 2026\"\n\n",
        "Links: [{\"url\":\"https://example.com/a\"},{\"url\":\"https://example.com/b\"}]\n\n",
        "Links: [{\"url\":\"https://example.com/b\"},{\"url\":\"https://example.com/c\"}]\n\n",
        "No links found.\n\n",
        "Saya sudah melakukan web search untuk kueri itu. Hasil paling relevan yang saya temukan:\n",
        "1. Contoh hasil pertama\n",
        "Link: https://example.com/a\n\n",
        "Kalau mau, saya bisa lanjutkan dengan rangkuman detail.\n\n",
        "REMINDER: You MUST include the sources above in your response to the user using markdown hyperlinks."
    );
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
                            "id": "call_ws_1",
                            "name": "WebSearch",
                            "input": {
                                "query": "berita reksadana terbaru Indonesia April 2026"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "call_ws_1",
                            "content": raw_output
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
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );

    let output = input[1]
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("tool result output should be a string");
    assert!(
        output.len() < raw_output.len(),
        "normalized output should be smaller than the raw verbose payload"
    );
    let output: serde_json::Value =
        serde_json::from_str(output).expect("normalized output should be valid JSON");
    assert_eq!(
        output.get("query").and_then(serde_json::Value::as_str),
        Some("berita reksadana terbaru Indonesia April 2026")
    );
    assert_eq!(
        output
            .get("content_blocks")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("url").and_then(serde_json::Value::as_str))
                    .collect::<Vec<_>>()
            }),
        Some(vec![
            "https://example.com/a",
            "https://example.com/b",
            "https://example.com/c"
        ])
    );
    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some(
            "Saya sudah melakukan web search untuk kueri itu. Hasil paling relevan yang saya temukan:\n1. Contoh hasil pertama"
        )
    );
}

#[test]
fn runtime_proxy_anthropic_reasoning_effort_normalizes_output_config_levels() {
    let cases = [
        ("gpt-5.4", "low", Some("low")),
        ("gpt-5.4", "medium", Some("medium")),
        ("gpt-5.4", "high", Some("high")),
        ("gpt-5.4", "max", Some("xhigh")),
        ("gpt-5", "max", Some("high")),
        ("gpt-5.4", "HIGH", Some("high")),
        ("gpt-5.4", "unknown", None),
    ];

    for (target_model, input_effort, expected) in cases {
        let value = serde_json::json!({
            "output_config": {
                "effort": input_effort,
            }
        });
        assert_eq!(
            runtime_proxy_anthropic_reasoning_effort(&value, target_model).as_deref(),
            expected,
            "input effort {input_effort:?} normalized incorrectly for target model {target_model}"
        );
    }
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_max_effort_to_xhigh_for_supported_model() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.4",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "max",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
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
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("xhigh")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_keeps_max_effort_at_high_for_legacy_model() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "max",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
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
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("high")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_honors_reasoning_override_env() {
    let _effort_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_REASONING_EFFORT", "xhigh");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.2",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "low",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
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
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("xhigh")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_web_search_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "web_search"
            },
            "tools": [
                {
                    "type": "web_search_20260209",
                    "name": "web_search",
                    "allowed_domains": ["example.com"],
                    "user_location": {
                        "type": "approximate",
                        "country": "ID",
                        "city": "Jakarta",
                        "region": "Jakarta",
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
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );
    assert_eq!(
        body.get("include")
            .and_then(serde_json::Value::as_array)
            .and_then(|include| include.first())
            .and_then(serde_json::Value::as_str),
        Some("web_search_call.action.sources")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("web_search")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("filters"))
            .and_then(|filters| filters.get("allowed_domains"))
            .and_then(serde_json::Value::as_array)
            .and_then(|domains| domains.first())
            .and_then(serde_json::Value::as_str),
        Some("example.com")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("user_location"))
            .and_then(|location| location.get("country"))
            .and_then(serde_json::Value::as_str),
        Some("ID")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_claude_web_search_tool_name() {
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
                            },
                            "blocked_domains": {
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
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert!(
        body.get("include").is_none(),
        "generic Claude WebSearch tools should not enable Responses web_search include filters"
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert!(
        !translated.server_tools.needs_buffered_translation(),
        "generic Claude WebSearch tools must stay on the direct streaming/function path"
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_claude_web_fetch_tool_name() {
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
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebFetch")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebFetch")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_forces_implicit_web_search_tool_choice() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tools": [
                {
                    "type": "web_search_20250305",
                    "name": "web_search",
                    "allowed_domains": []
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Perform a web search for the latest OpenAI news"
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

    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("web_search")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_mcp_approval_response_block() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "mcp_approval_response",
                            "approval_request_id": "mcpapr_1",
                            "approve": true,
                            "reason": "Looks good"
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

    assert_eq!(input.len(), 1);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("mcp_approval_response")
    );
    assert_eq!(
        input[0]
            .get("approval_request_id")
            .and_then(serde_json::Value::as_str),
        Some("mcpapr_1")
    );
    assert_eq!(
        input[0].get("approve").and_then(serde_json::Value::as_bool),
        Some(true)
    );
}
