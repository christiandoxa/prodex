use super::*;

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
