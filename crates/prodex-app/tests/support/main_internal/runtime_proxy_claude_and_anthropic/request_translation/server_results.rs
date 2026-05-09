use super::*;

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
fn translate_runtime_anthropic_messages_request_preserves_file_backed_document_and_container_blocks()
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

