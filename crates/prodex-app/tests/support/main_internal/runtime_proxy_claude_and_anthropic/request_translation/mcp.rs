use super::*;

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
fn translate_runtime_anthropic_messages_request_falls_back_for_unrepresentable_mcp_toolset_denylist()
 {
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

