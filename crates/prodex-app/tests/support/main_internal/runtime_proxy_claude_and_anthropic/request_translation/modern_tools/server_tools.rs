use super::*;

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

