use super::*;

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
fn runtime_anthropic_sse_response_parts_from_responses_sse_bytes_preserves_carried_server_tool_usage()
 {
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
        RuntimeAnthropicServerToolUsage {
            web_search_requests: 1,
            web_fetch_requests: 1,
            code_execution_requests: 0,
            tool_search_requests: 1,
        },
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
