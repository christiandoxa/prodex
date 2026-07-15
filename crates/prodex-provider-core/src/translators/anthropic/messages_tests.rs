use super::*;
use crate::{ProviderTransformLoss, anthropic_messages_translator};

fn request(value: Value) -> ProviderTransformResult {
    anthropic_messages_translator().transform_request(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&value).unwrap(),
    ))
}

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
            {"type": "function_call", "call_id": "call_test", "name": "read_file", "arguments": "{\"path\":\"/tmp/test\"}"},
            {"type": "function_call_output", "call_id": "call_test", "output": "contents"}
        ]
    }));
    assert!(matches!(result.loss, ProviderTransformLoss::Lossless));
    assert_eq!(result.to_format, ProviderWireFormat::AnthropicMessages);
    let body: Value = serde_json::from_slice(result.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["system"], "Be concise.");
    assert_eq!(body["max_tokens"], 512);
    assert_eq!(body["tool_choice"]["type"], "any");
    assert_eq!(body["tools"][0]["input_schema"]["required"][0], "path");
    assert_eq!(body["messages"][1]["content"][0]["type"], "tool_use");
    assert_eq!(body["messages"][2]["content"][0]["type"], "tool_result");
    assert_eq!(
        body["messages"][2]["content"][0]["tool_use_id"],
        "call_test"
    );
}

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

#[test]
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
    assert!(body.contains("event: response.output_text.delta"));
    assert!(body.contains("\"delta\":\"hello\""));
}
