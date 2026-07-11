use super::*;

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

