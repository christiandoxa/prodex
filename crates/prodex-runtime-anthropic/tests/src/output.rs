use super::*;
use std::cell::Cell;
use std::io::{Cursor, Read};

fn test_messages_request() -> RuntimeAnthropicMessagesRequest {
    let mut server_tools = RuntimeAnthropicServerTools::default();
    server_tools.register("web_search", "web_search");
    RuntimeAnthropicMessagesRequest {
        translated_request: RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers: Vec::new(),
            body: b"{}".to_vec(),
        },
        requested_model: "gpt-5.4".to_string(),
        stream: false,
        want_thinking: false,
        server_tools,
        carried_web_search_requests: 0,
        carried_web_fetch_requests: 0,
        carried_code_execution_requests: 0,
        carried_tool_search_requests: 0,
    }
}

#[test]
fn anthropic_first_event_retry_is_bounded_and_precommit_only() {
    assert!(runtime_anthropic_first_event_retry_allowed(0, false));
    assert!(!runtime_anthropic_first_event_retry_allowed(1, false));
    assert!(!runtime_anthropic_first_event_retry_allowed(0, true));
}

#[test]
fn server_tool_followup_translation_passes_upstream_errors_without_followup() {
    let observed = Cell::new(false);
    let result =
        translate_runtime_buffered_responses_reply_to_anthropic_with_server_tool_followups(
            RuntimeBufferedResponseParts {
                status: 429,
                headers: vec![("content-type".to_string(), b"text/plain".to_vec())],
                body: b"too many requests".to_vec(),
            },
            &test_messages_request(),
            2,
            |observation| {
                observed.set(true);
                assert_eq!(observation.status, 429);
                assert_eq!(observation.content_type, Some("text/plain"));
                assert_eq!(observation.followup_attempt, 0);
            },
            |_| panic!("upstream errors should not trigger server-tool followup"),
        )
        .unwrap();

    assert!(observed.get());
    match result {
        RuntimeResponsesReply::Buffered(parts) => assert_eq!(parts.status, 429),
        RuntimeResponsesReply::Streaming(_) => panic!("translation should be buffered"),
    }
}

#[test]
fn anthropic_streaming_conversion_preserves_incomplete_failed_and_eof_errors() {
    for upstream in [
        "data: {\"type\":\"response.incomplete\",\"response\":{}}\n\n",
        "data: {\"type\":\"response.failed\",\"response\":{}}\n\n",
        "",
    ] {
        let mut reader = RuntimeAnthropicSseReader::new(
            Box::new(Cursor::new(upstream.as_bytes())),
            "claude-sonnet-4-6".to_string(),
            false,
            RuntimeAnthropicServerToolUsage::default(),
            RuntimeAnthropicServerTools::default(),
        );
        let mut body = String::new();
        reader.read_to_string(&mut body).unwrap();

        assert!(body.contains("event: error"), "{body}");
        assert!(!body.contains("\"stop_reason\":\"end_turn\""), "{body}");
    }
}

#[test]
fn anthropic_buffered_conversion_requires_a_completed_event() {
    for upstream in [
        b"data: {\"type\":\"response.incomplete\",\"response\":{}}\n\n".as_slice(),
        b"data: {\"type\":\"response.failed\",\"response\":{}}\n\n".as_slice(),
        b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n".as_slice(),
    ] {
        assert!(
            runtime_anthropic_response_from_sse_bytes(upstream, "claude-sonnet-4-6", false)
                .is_err()
        );
    }
}

#[test]
fn responses_terminal_events_preserve_anthropic_failure_and_max_tokens() {
    let max_tokens = format!(
        "event: response.incomplete\ndata: {}\n\n",
        serde_json::json!({
            "type": "response.incomplete",
            "response": {
                "status": "incomplete",
                "incomplete_details": {
                    "reason": "max_output_tokens",
                    "message": "limit"
                },
                "usage": {"input_tokens": 2, "output_tokens": 3}
            }
        })
    );
    let mut reader = RuntimeAnthropicSseReader::new(
        Box::new(Cursor::new(max_tokens.as_bytes().to_vec())),
        "claude-sonnet-4".to_string(),
        false,
        RuntimeAnthropicServerToolUsage::default(),
        RuntimeAnthropicServerTools::default(),
    );
    let mut streamed = String::new();
    reader
        .read_to_string(&mut streamed)
        .expect("Anthropic stream should translate");
    assert!(streamed.contains("\"stop_reason\":\"max_tokens\""));
    assert!(!streamed.contains("\"stop_reason\":\"end_turn\""));

    let failed = format!(
        "event: response.failed\ndata: {}\n\n",
        serde_json::json!({
            "type": "response.failed",
            "response": {
                "status": "failed",
                "error": {"message": "ACP failed"}
            }
        })
    );
    let mut reader = RuntimeAnthropicSseReader::new(
        Box::new(Cursor::new(failed.as_bytes().to_vec())),
        "claude-sonnet-4".to_string(),
        false,
        RuntimeAnthropicServerToolUsage::default(),
        RuntimeAnthropicServerTools::default(),
    );
    let mut failed_stream = String::new();
    reader
        .read_to_string(&mut failed_stream)
        .expect("Anthropic failure stream should translate");
    assert!(failed_stream.contains("\"type\":\"error\""));
    assert!(failed_stream.contains("ACP failed"));

    let buffered =
        runtime_anthropic_response_from_sse_bytes(max_tokens.as_bytes(), "claude-sonnet-4", false)
            .expect("buffered Anthropic response should translate");
    assert_eq!(buffered["stop_reason"], "max_tokens");
}

#[test]
fn buffered_json_terminal_states_do_not_become_success() {
    let request = test_messages_request();
    let translate = |body: serde_json::Value| {
        translate_runtime_buffered_responses_reply_to_anthropic(
            RuntimeBufferedResponseParts {
                status: 200,
                headers: vec![("content-type".to_string(), b"application/json".to_vec())],
                body: serde_json::to_vec(&body).unwrap(),
            },
            &request,
        )
    };

    let reply = translate(serde_json::json!({
        "status": "incomplete",
        "incomplete_details": {"reason": "max_output_tokens"},
        "output": [],
    }))
    .expect("max-token response should translate");
    let RuntimeResponsesReply::Buffered(parts) = reply else {
        panic!("translation should be buffered");
    };
    let message: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
    assert_eq!(message["stop_reason"], "max_tokens");

    assert!(
        translate(serde_json::json!({
            "status": "failed",
            "error": {"message": "provider failed"},
        }))
        .err()
        .expect("failed response should be rejected")
        .to_string()
        .contains("provider failed")
    );
    assert!(
        translate(serde_json::json!({
            "status": "incomplete",
            "incomplete_details": {"reason": "content_filter", "message": "filtered"},
        }))
        .err()
        .expect("non-token incomplete response should be rejected")
        .to_string()
        .contains("filtered")
    );
}

#[cfg(feature = "mojo")]
#[test]
fn mojo_anthropic_kernel_preserves_response_and_tool_wire_shapes() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "output": [
                {"type": "message", "content": [{"type": "output_text", "text": "hello"}]},
                {"type": "function_call", "call_id": "call_1", "name": "lookup", "arguments": "{\"q\":\"x\"}"}
            ],
            "usage": {
                "input_tokens": 2,
                "output_tokens": 3,
                "input_tokens_details": {"cached_tokens": 1}
            }
        }),
        "gpt-5.4",
        false,
    );
    assert_eq!(
        response["content"][0],
        serde_json::json!({
            "type": "text",
            "text": "hello"
        })
    );
    assert_eq!(
        response["content"][1],
        serde_json::json!({
            "type": "tool_use",
            "id": "call_1",
            "name": "lookup",
            "input": {"q": "x"}
        })
    );
    assert_eq!(response["stop_reason"], "tool_use");
    assert_eq!(response["usage"]["cache_read_input_tokens"], 1);

    let sse = runtime_anthropic_sse_response_parts_from_message_value(response);
    assert_eq!(sse.status, 200);
    assert_eq!(
        sse.headers,
        vec![("Content-Type".to_string(), b"text/event-stream".to_vec())]
    );
    for line in String::from_utf8(sse.body)
        .expect("Mojo SSE output is UTF-8")
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
    {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|error| panic!("every Mojo SSE data frame is JSON: {line}: {error}"));
    }
}

#[cfg(feature = "mojo")]
#[test]
fn mojo_anthropic_kernel_preserves_input_and_server_tool_shapes() {
    let tool_call = runtime_proxy_translate_anthropic_tool_call(&serde_json::json!({
        "type": "mcp_tool_use",
        "id": "call_mcp",
        "name": "search",
        "server_name": "docs",
        "input": {"term": "rust"}
    }))
    .expect("MCP tool call should translate");
    assert_eq!(tool_call["type"], "function_call");
    assert_eq!(tool_call["call_id"], "call_mcp");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(tool_call["arguments"].as_str().unwrap())
            .unwrap()["server_name"],
        "docs"
    );

    let result = runtime_proxy_translate_anthropic_tool_result(&serde_json::json!({
        "type": "tool_result",
        "tool_use_id": "call_mcp",
        "content": "done"
    }))
    .expect("tool result should translate");
    assert_eq!(
        result[0],
        serde_json::json!({
            "type": "function_call_output",
            "call_id": "call_mcp",
            "output": "done"
        })
    );

    let mcp = runtime_anthropic_mcp_call_blocks_from_output_item(&serde_json::json!({
        "id": "call_mcp",
        "name": "search",
        "server_label": "docs",
        "arguments": "{\"term\":\"rust\"}",
        "output": "done"
    }));
    assert_eq!(mcp[0]["type"], "mcp_tool_use");
    assert_eq!(mcp[1]["type"], "mcp_tool_result");
    assert_eq!(mcp[1]["is_error"], false);

    let mut server_tools = RuntimeAnthropicServerTools::default();
    server_tools.register_with_block_type("web_search", "web_search", "server_tool_use");
    let server = runtime_anthropic_server_tool_use_block(
        "call_web",
        "web_search",
        serde_json::json!({"query": "rust"}),
        Some(&server_tools),
    )
    .expect("registered server tool should translate");
    assert_eq!(server["type"], "server_tool_use");
    assert_eq!(server["name"], "web_search");

    let approval =
        runtime_anthropic_mcp_approval_request_block_from_output_item(&serde_json::json!({
            "id": "approval_1",
            "name": "search",
            "server_label": "docs",
            "arguments": "{\"term\":\"rust\"}"
        }));
    assert_eq!(approval["type"], "mcp_approval_request");
    assert_eq!(approval["input"]["term"], "rust");
}

#[cfg(feature = "mojo")]
#[test]
fn mojo_anthropic_kernel_preserves_native_tool_results_and_usage() {
    let shell = runtime_proxy_translate_anthropic_shell_tool_result(
        &serde_json::json!({
            "tool_use_id": "shell_1",
            "content": "stdout",
            "is_error": false
        }),
        Some(128),
    )
    .expect("shell result should translate");
    assert_eq!(shell[0]["type"], "shell_call_output");
    assert_eq!(shell[0]["output"][0]["stdout"], "stdout");
    assert_eq!(shell[0]["max_output_length"], 128);

    let computer = runtime_proxy_translate_anthropic_computer_tool_result(&serde_json::json!({
        "tool_use_id": "computer_1",
        "content": [{"type": "image", "source": {
            "type": "base64", "media_type": "image/png", "data": "AQI="
        }}]
    }))
    .expect("computer result should translate")
    .expect("valid screenshot should be emitted");
    assert_eq!(computer[0]["type"], "computer_call_output");
    assert_eq!(
        computer[0]["output"]["image_url"],
        "data:image/png;base64,AQI="
    );

    assert_eq!(
        runtime_anthropic_usage_json(2, 3, Some(1), 4, 5, 6, 7),
        serde_json::Map::from_iter([
            ("input_tokens".to_string(), serde_json::json!(2)),
            ("output_tokens".to_string(), serde_json::json!(3)),
            ("cache_read_input_tokens".to_string(), serde_json::json!(1)),
            (
                "server_tool_use".to_string(),
                serde_json::json!({
                    "web_search_requests": 4,
                    "web_fetch_requests": 5,
                    "code_execution_requests": 6,
                    "tool_search_requests": 7
                })
            )
        ])
    );
}

#[cfg(feature = "mojo")]
#[test]
fn mojo_anthropic_kernel_preserves_thinking_images_and_mcp_sse_blocks() {
    assert_eq!(
        runtime_anthropic_sse_event_bytes("ping", serde_json::json!({"ok": true})),
        b"event: ping\ndata: {\"ok\":true}\n\n"
    );

    let content = runtime_proxy_translate_anthropic_user_content_blocks(&[
        serde_json::json!({"type": "text", "text": "before"}),
        serde_json::json!({
            "type": "image",
            "source": {"type": "base64", "media_type": "image/png", "data": "AQI="}
        }),
        serde_json::json!({"type": "text", "text": "after"}),
    ])
    .expect("image content should be represented as input parts");
    assert_eq!(content[0]["type"], "input_text");
    assert_eq!(content[1]["type"], "input_image");
    assert_eq!(content[2]["type"], "input_text");

    let message = serde_json::json!({
        "id": "msg_sse",
        "model": "gpt-5.4",
        "content": [
            {"type": "thinking", "thinking": "plan"},
            {"type": "mcp_tool_use", "id": "mcp_1", "name": "search", "server_name": "docs", "input": {"q": "rust"}},
            {"type": "mcp_tool_result", "tool_use_id": "mcp_1", "is_error": false, "content": [{"type": "text", "text": "done"}]}
        ],
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 1, "output_tokens": 2}
    });
    let body =
        String::from_utf8(runtime_anthropic_sse_response_parts_from_message_value(message).body)
            .expect("Mojo SSE output is UTF-8");
    assert!(body.contains("\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}"));
    assert!(body.contains("\"type\":\"mcp_tool_use\""));
    assert!(body.contains("\"type\":\"mcp_tool_result\""));
    for line in body.lines().filter_map(|line| line.strip_prefix("data: ")) {
        serde_json::from_str::<serde_json::Value>(line).expect("every SSE block is JSON");
    }
}
