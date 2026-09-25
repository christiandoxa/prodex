use super::*;
use crate::{ProviderTransformLoss, anthropic_messages_translator};
#[cfg(feature = "mojo")]
use serde_json::{Value, json};

#[cfg(feature = "mojo")]
#[test]
fn mojo_response_envelope_matches_expected_fixtures() {
    let output = vec![json!({"type": "message", "content": [{"text": "x\n\u{1f980}"}]})];
    let cases = [
        (
            json!({}),
            json!({
                "id": "resp_anthropic",
                "object": "response",
                "created_at": 123,
                "model": "unknown",
                "output": output.clone(),
            }),
        ),
        (
            json!({
                "id": "msg_\u{1f980}",
                "model": "claude",
                "usage": {
                    "input_tokens": 9,
                    "output_tokens": 4,
                    "server_tool_use": {"web_search_requests": 2},
                },
                "stop_reason": null,
            }),
            json!({
                "id": "msg_\u{1f980}",
                "object": "response",
                "created_at": 123,
                "model": "claude",
                "output": output.clone(),
                "usage": {"input_tokens": 9, "output_tokens": 4, "total_tokens": 13},
                "tool_usage": {"web_search": {"num_requests": 2}},
                "metadata": {"anthropic": {"stop_reason": null}},
            }),
        ),
        (
            json!({
                "id": null,
                "model": 1,
                "usage": {"input_tokens": u64::MAX, "output_tokens": 1},
                "stop_reason": "end_turn",
            }),
            json!({
                "id": "resp_anthropic",
                "object": "response",
                "created_at": 123,
                "model": "unknown",
                "output": output.clone(),
                "usage": {"input_tokens": u64::MAX, "output_tokens": 1, "total_tokens": u64::MAX},
                "metadata": {"anthropic": {"stop_reason": "end_turn"}},
            }),
        ),
    ];
    for (value, expected) in cases {
        assert_eq!(
            anthropic_response_envelope_mojo(&value, output.clone(), 123).unwrap(),
            expected,
            "{value}"
        );
    }
}

#[cfg(feature = "mojo")]
fn request(value: Value) -> ProviderTransformResult {
    anthropic_messages_translator().transform_request(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&value).unwrap(),
    ))
}

#[cfg(feature = "mojo")]
#[test]
fn request_maps_system_tools_and_tool_history_to_native_messages() {
    let result = request(json!({
        "model": "claude-sonnet-4-6",
        "instructions": "Be concise.",
        "max_output_tokens": 512,
        "stream": true,
        "tools": [{
            "type": "function",
            "name": "read_file",
            "description": "Read one file",
            "parameters": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}
        }],
        "tool_choice": "required",
        "input": [
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Read it"}]},
            {"type": "function_call", "call_id": "call_test", "name": "read_file", "arguments": "{\"path\":\"/home/test-user/test\"}"},
            {"type": "function_call_output", "call_id": "call_test", "output": "contents"}
        ]
    }));
    assert!(matches!(result.loss, ProviderTransformLoss::Lossless));
    assert_eq!(result.to_format, ProviderWireFormat::AnthropicMessages);
    let body: Value = serde_json::from_slice(result.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        body,
        json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "Read it"}]},
                {"role": "assistant", "content": [{
                    "type": "tool_use",
                    "id": "call_test",
                    "name": "read_file",
                    "input": {"path": "/home/test-user/test"}
                }]},
                {"role": "user", "content": [{
                    "type": "tool_result",
                    "tool_use_id": "call_test",
                    "content": "contents"
                }]}
            ],
            "max_tokens": 512,
            "stream": true,
            "system": "Be concise.",
            "tools": [{
                "name": "read_file",
                "description": "Read one file",
                "input_schema": {
                    "type": "object",
                    "properties": {"path": {"type": "string"}},
                    "required": ["path"]
                }
            }],
            "tool_choice": {"type": "any"}
        })
    );
}

#[cfg(feature = "mojo")]
#[test]
fn request_rejects_unmappable_sampling_fields() {
    let result = request(json!({
        "model": "claude-sonnet-4-6",
        "input": "hello",
        "presence_penalty": 0.5
    }));
    assert!(matches!(
        result.loss,
        ProviderTransformLoss::Rejected { .. }
    ));
    assert!(result.body.is_none());
}

#[cfg(feature = "mojo")]
#[test]
fn chat_request_rejects_every_unmapped_top_level_field() {
    for (field, value) in [
        ("response_format", json!({"type": "json_object"})),
        ("user_id", json!("user-test")),
        ("top_logprobs", json!(2)),
        ("logprobs", json!(true)),
        ("presence_penalty", json!(0.5)),
        ("frequency_penalty", json!(0.5)),
        ("seed", json!(7)),
        ("unknown_field", json!(true)),
    ] {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "messages": [{"role": "user", "content": "hello"}],
            "stream": false,
        });
        body[field] = value;
        let result = translate_chat_request_to_anthropic(ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            serde_json::to_vec(&body).unwrap(),
        ));
        assert!(
            matches!(
                &result.loss,
                ProviderTransformLoss::Rejected { reason } if reason.contains(field)
            ),
            "field {field} was not rejected: {:?}",
            result.loss
        );
        assert!(result.body.is_none());
    }
}

#[cfg(feature = "mojo")]
#[test]
fn chat_request_accepts_benign_ignored_transport_fields() {
    let result = translate_chat_request_to_anthropic(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&json!({
            "model": "claude-sonnet-4-6",
            "messages": [{"role": "user", "content": "hello"}],
            "stream": true,
            "parallel_tool_calls": true,
            "stream_options": {"include_usage": true},
        }))
        .unwrap(),
    ));
    assert!(matches!(result.loss, ProviderTransformLoss::Lossless));
}

#[cfg(feature = "mojo")]
#[test]
fn chat_request_normalizes_namespaced_tools_in_the_authoritative_path() {
    let result = translate_chat_request_to_anthropic(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&json!({
            "messages": [{"role": "user", "content": "hello"}],
            "tools": [
                {"name": "functions.lookup", "parameters": {"type": "object"}},
                {"name": "search", "namespace": "tools", "parameters": {"type": "object"}}
            ],
            "tool_choice": {"type": "function", "name": "functions.lookup"}
        }))
        .unwrap(),
    ));
    let body: Value = serde_json::from_slice(result.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["tools"][0]["name"], "functions--lookup");
    assert_eq!(body["tools"][1]["name"], "tools--search");
    assert_eq!(body["tool_choice"]["name"], "functions--lookup");
}

#[cfg(feature = "mojo")]
#[test]
fn chat_request_maps_native_web_search_tool_and_reports_context_degradation() {
    let result = translate_chat_request_to_anthropic(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&json!({
            "model": "deepseek-chat",
            "messages": [{"role": "user", "content": "find it"}],
            "web_search_options": {
                "search_context_size": "high",
                "allowed_domains": ["example.com"],
                "max_uses": 3,
                "user_location": {"type": "approximate", "country": "US"}
            }
        }))
        .unwrap(),
    ));
    assert!(matches!(
        result.loss,
        ProviderTransformLoss::DegradedButSafe { .. }
    ));
    let body: Value = serde_json::from_slice(result.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["tools"][0]["type"], "web_search_20250305");
    assert_eq!(body["tools"][0]["name"], "web_search");
    assert_eq!(body["tools"][0]["allowed_domains"][0], "example.com");
    assert_eq!(body["tools"][0]["max_uses"], 3);
    assert!(body["tools"][0].get("search_context_size").is_none());
}

#[cfg(feature = "mojo")]
#[test]
fn chat_request_rejects_disabled_parallel_tool_calls() {
    let result = translate_chat_request_to_anthropic(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&json!({
            "model": "claude-sonnet-4-6",
            "messages": [{"role": "user", "content": "hello"}],
            "parallel_tool_calls": false,
        }))
        .unwrap(),
    ));
    assert!(matches!(
        result.loss,
        ProviderTransformLoss::Rejected { .. }
    ));
    assert!(result.body.is_none());
}

#[cfg(not(feature = "mojo"))]
#[test]
fn request_translation_is_explicitly_unsupported_without_mojo() {
    let response = anthropic_messages_translator().transform_request(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        b"invalid JSON is not parsed without the request kernel".to_vec(),
    ));
    let chat = crate::translate_openai_chat_request_to_anthropic_messages(
        ProviderTransformInput::new(ProviderEndpoint::Responses, b"{}".to_vec()),
    );
    for (result, from_format) in [
        (response, ProviderWireFormat::OpenAiResponses),
        (chat, ProviderWireFormat::OpenAiChatCompletions),
    ] {
        assert!(matches!(
            result.loss,
            ProviderTransformLoss::UnsupportedUpstream { ref reason }
                if reason == "Anthropic Messages request translation requires Mojo support"
        ));
        assert!(result.body.is_none());
        assert_eq!(result.from_format, from_format);
        assert_eq!(result.to_format, ProviderWireFormat::AnthropicMessages);
    }
}

#[test]
#[cfg(feature = "mojo")]
fn response_maps_text_tools_and_usage_to_responses() {
    let result = anthropic_messages_translator().transform_response(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&json!({
            "id": "msg_test",
            "type": "message",
            "role": "assistant",
            "model": "claude-sonnet-4-6",
            "content": [
                {"type": "text", "text": "checking"},
                {"type": "tool_use", "id": "call_test", "name": "functions--read_file", "input": {"path": "/tmp/test"}}
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 9, "output_tokens": 4}
        }))
        .unwrap(),
    ));
    assert!(matches!(result.loss, ProviderTransformLoss::Lossless));
    let body: Value = serde_json::from_slice(result.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["id"], "msg_test");
    assert_eq!(body["output"][0]["content"][0]["text"], "checking");
    assert_eq!(body["output"][1]["namespace"], "functions");
    assert_eq!(body["output"][1]["name"], "read_file");
    assert_eq!(body["usage"]["total_tokens"], 13);
}

#[test]
#[cfg(feature = "mojo")]
fn response_maps_native_web_search_call_sources_and_usage() {
    let result = anthropic_messages_translator().transform_response(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&json!({
            "id": "msg_search",
            "model": "deepseek-chat",
            "content": [
                {"type": "server_tool_use", "id": "srv_1", "name": "web_search", "input": {"queries": ["current release", null, 7, "policy"]}},
                {"type": "web_search_tool_result", "tool_use_id": "srv_1", "content": [
                    {"type": "web_search_result", "url": "https://example.com/release", "title": "Release"}
                ]},
                {"type": "text", "text": "Found it."}
            ],
            "usage": {
                "input_tokens": 5,
                "output_tokens": 2,
                "server_tool_use": {"web_search_requests": 1}
            }
        }))
        .unwrap(),
    ));
    assert!(matches!(result.loss, ProviderTransformLoss::Lossless));
    let body: Value = serde_json::from_slice(result.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["output"][0]["type"], "web_search_call");
    assert_eq!(body["output"][0]["status"], "completed");
    assert_eq!(
        body["output"][0]["action"]["queries"],
        json!(["current release", "policy"])
    );
    assert_eq!(
        body["output"][0]["action"]["sources"][0]["url"],
        "https://example.com/release"
    );
    assert_eq!(body["tool_usage"]["web_search"]["num_requests"], 1);
    assert_eq!(body["output"][1]["content"][0]["text"], "Found it.");
}

#[test]
#[cfg(feature = "mojo")]
fn response_plan_preserves_flush_and_web_search_result_order() {
    let result = anthropic_messages_translator().transform_response(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&json!({
            "id": "msg_order",
            "content": [
                {"type": "text", "text": "before"},
                {"type": "thinking"},
                {"type": "web_search_tool_result", "tool_use_id": "srv_later", "content": [
                    {"type": "web_search_result", "url": "https://example.com/later"}
                ]},
                {"type": "server_tool_use", "id": "srv_later", "name": "web_search", "input": {"query": "later"}},
                {"type": "text", "text": "after"}
            ]
        }))
        .unwrap(),
    ));
    let body: Value = serde_json::from_slice(result.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["output"].as_array().unwrap().len(), 3);
    assert_eq!(body["output"][0]["content"][0]["text"], "before");
    assert_eq!(body["output"][1]["type"], "web_search_call");
    assert!(
        body["output"][1]["action"]["sources"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(body["output"][2]["content"][0]["text"], "after");
}

#[test]
#[cfg(feature = "mojo")]
fn stream_maps_native_delta_and_tolerates_ping() {
    let ping = anthropic_messages_translator().transform_stream_event(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        b"event: ping\ndata: {\"type\":\"ping\"}\n\n".to_vec(),
    ));
    assert!(matches!(ping.loss, ProviderTransformLoss::Lossless));
    assert_eq!(ping.body.as_deref(), Some([].as_slice()));

    let delta = anthropic_messages_translator().transform_stream_event(
        ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\n".to_vec(),
        ),
    );
    let body = String::from_utf8(delta.body.unwrap()).unwrap();
    assert_eq!(
        body.lines().next(),
        Some("event: response.output_text.delta")
    );
    let expected = json!({
        "type": "response.output_text.delta",
        "output_index": 0,
        "delta": "hello",
    });
    assert_eq!(
        serde_json::from_str::<Value>(
            body.lines()
                .find_map(|line| line.strip_prefix("data: "))
                .unwrap(),
        )
        .unwrap(),
        expected
    );
}

#[test]
#[cfg(feature = "mojo")]
fn response_preserves_escaped_text_and_reasoning() {
    let result = anthropic_messages_translator().transform_response(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&json!({
            "content": [
                {"type": "text", "text": "line \"one\"\nline two"},
                {"type": "thinking", "thinking": "hidden \"step\""},
                {"type": "text", "text": "done"}
            ]
        }))
        .unwrap(),
    ));
    let body: Value = serde_json::from_slice(result.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        body["output"][0]["content"][0]["text"],
        "line \"one\"\nline two"
    );
    assert_eq!(body["output"][1]["type"], "reasoning");
    assert_eq!(body["output"][1]["summary"][0]["text"], "hidden \"step\"");
    assert_eq!(body["output"][2]["content"][0]["text"], "done");
}

#[cfg(not(feature = "mojo"))]
#[test]
fn response_translation_is_explicitly_unsupported_without_mojo() {
    let result = anthropic_messages_translator().transform_response(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        br#"{"content":[]}"#.to_vec(),
    ));
    assert!(matches!(
        result.loss,
        ProviderTransformLoss::UnsupportedUpstream { ref reason }
            if reason == "Anthropic Messages response translation requires Mojo support"
    ));
    assert!(result.body.is_none());
    assert_eq!(result.from_format, ProviderWireFormat::AnthropicMessages);
    assert_eq!(result.to_format, ProviderWireFormat::OpenAiResponses);
}

#[test]
#[cfg(feature = "mojo")]
fn stream_maps_tool_search_and_completion_events() {
    let tool = anthropic_messages_translator().transform_stream_event(
        ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            b"event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":2,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call_stream\",\"name\":\"read_file\"}}\n\n".to_vec(),
        ),
    );
    let tool = String::from_utf8(tool.body.unwrap()).unwrap();
    assert!(tool.contains("\"type\":\"function_call\""));
    assert!(tool.contains("\"call_id\":\"call_stream\""));

    let search = anthropic_messages_translator().transform_stream_event(
        ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            b"event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":3,\"content_block\":{\"type\":\"server_tool_use\",\"id\":\"srv_stream\",\"name\":\"web_search\",\"input\":{\"query\":\"current release\"}}}\n\n".to_vec(),
        ),
    );
    let search: Value = serde_json::from_str(
        String::from_utf8(search.body.unwrap())
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(search["item"]["status"], "in_progress");
    assert_eq!(search["item"]["action"]["queries"][0], "current release");

    let completed =
        anthropic_messages_translator().transform_stream_event(ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_vec(),
        ));
    assert_eq!(
        String::from_utf8(completed.body.unwrap()).unwrap(),
        "event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n"
    );
}

#[cfg(feature = "mojo")]
#[test]
fn stream_rejects_malformed_sse_without_changing_wire_formats() {
    let malformed =
        anthropic_messages_translator().transform_stream_event(ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            b"event: content_block_delta\ndata: {\n\n".to_vec(),
        ));
    assert!(matches!(
        malformed.loss,
        ProviderTransformLoss::Rejected { ref reason }
            if reason.starts_with("failed to parse Anthropic SSE JSON:")
    ));
    assert_eq!(malformed.from_format, ProviderWireFormat::AnthropicMessages);
    assert_eq!(malformed.to_format, ProviderWireFormat::OpenAiResponses);
    assert!(malformed.body.is_none());

    let unframed = anthropic_messages_translator().transform_stream_event(
        ProviderTransformInput::new(ProviderEndpoint::Responses, b"{\"type\":\"ping\"}".to_vec()),
    );
    assert!(matches!(
        unframed.loss,
        ProviderTransformLoss::UnsupportedUpstream { ref reason }
            if reason == "Anthropic SSE event must contain data: <json> framing"
    ));
    assert_eq!(unframed.from_format, ProviderWireFormat::AnthropicMessages);
    assert_eq!(unframed.to_format, ProviderWireFormat::OpenAiResponses);
    assert!(unframed.body.is_none());
}

#[cfg(not(feature = "mojo"))]
#[test]
fn stream_translation_is_explicitly_unsupported_without_mojo() {
    let result =
        anthropic_messages_translator().transform_stream_event(ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            b"not SSE or JSON; translation must not parse this body".to_vec(),
        ));
    assert!(matches!(
        result.loss,
        ProviderTransformLoss::UnsupportedUpstream { ref reason }
            if reason == "Anthropic Messages stream translation requires Mojo support"
    ));
    assert_eq!(result.from_format, ProviderWireFormat::AnthropicMessages);
    assert_eq!(result.to_format, ProviderWireFormat::OpenAiResponses);
    assert!(result.body.is_none());
}
