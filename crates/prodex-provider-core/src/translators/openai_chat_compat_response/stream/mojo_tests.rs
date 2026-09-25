use super::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderWireFormat,
    translate_chat_stream_event_to_responses,
};
use crate::ProviderTransformLoss;
use serde_json::{Value, json};

fn translate(value: Value) -> crate::ProviderTransformResult {
    let data = serde_json::to_string(&value).expect("event fixture serializes");
    let event = format!("data: {data}\n\n");
    translate_chat_stream_event_to_responses(
        ProviderId::Anthropic,
        ProviderTransformInput::new(ProviderEndpoint::Responses, event),
    )
}

fn assert_event(result: crate::ProviderTransformResult, event_name: &str, expected: Value) {
    assert_eq!(result.loss, ProviderTransformLoss::Lossless);
    assert_eq!(result.endpoint, ProviderEndpoint::Responses);
    assert_eq!(
        result.from_format,
        ProviderWireFormat::OpenAiChatCompletions
    );
    assert_eq!(result.to_format, ProviderWireFormat::OpenAiResponses);
    let body = String::from_utf8(result.body.expect("translated SSE body"))
        .expect("translated SSE is UTF-8");
    let (header, data) = body.split_once('\n').expect("SSE header and data line");
    assert_eq!(header, format!("event: {event_name}"));
    let payload = data
        .strip_suffix("\n\n")
        .and_then(|line| line.strip_prefix("data: "))
        .expect("SSE data framing");
    assert_eq!(serde_json::from_str::<Value>(payload).unwrap(), expected);
}

#[test]
fn tool_argument_delta_takes_precedence_over_text_delta() {
    assert_event(
        translate(json!({
            "choices": [{"delta": {
                "content": "ignored",
                "tool_calls": [{"id": "call_test", "function": {
                    "name": "functions.exec_command",
                    "arguments": r#"{"cmd":"ls"}"#
                }}]
            }}]
        })),
        "response.function_call_arguments.delta",
        json!({
            "type": "response.function_call_arguments.delta",
            "call_id": "call_test",
            "delta": r#"{"cmd":"rtk ls"}"#
        }),
    );
}

#[test]
fn text_delta_fixture_preserves_unicode_and_escaping() {
    assert_event(
        translate(json!({
            "choices": [{"delta": {"content": "東京\n\"quoted\""}}]
        })),
        "response.output_text.delta",
        json!({
            "type": "response.output_text.delta",
            "delta": "東京\n\"quoted\""
        }),
    );
}

#[test]
fn finish_and_done_events_emit_completion() {
    assert_event(
        translate(json!({"choices": [{"delta": {}, "finish_reason": "stop"}]})),
        "response.completed",
        json!({}),
    );

    let done = translate_chat_stream_event_to_responses(
        ProviderId::Anthropic,
        ProviderTransformInput::new(ProviderEndpoint::Responses, b"data: [DONE]\n\n"),
    );
    assert_event(done, "response.completed", json!({}));
}

#[test]
fn event_without_supported_delta_remains_unsupported() {
    let result = translate(json!({"choices": [{"delta": {}}]}));
    assert_eq!(
        result.loss,
        ProviderTransformLoss::UnsupportedUpstream {
            reason: "chat completions SSE event does not contain a supported text delta".into(),
        }
    );
    assert_eq!(result.body, None);
}
