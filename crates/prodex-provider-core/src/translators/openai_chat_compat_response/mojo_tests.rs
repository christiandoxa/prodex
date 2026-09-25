use super::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderWireFormat,
    translate_chat_response_to_responses_at,
};
use crate::ProviderTransformLoss;
use serde_json::{Value, json};

fn translate(response: Value, now_secs: u64) -> Value {
    let result = translate_chat_response_to_responses_at(
        ProviderId::Anthropic,
        ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            serde_json::to_vec(&response).expect("response fixture serializes"),
        ),
        now_secs,
    );
    assert_eq!(result.loss, ProviderTransformLoss::Lossless);
    assert_eq!(result.endpoint, ProviderEndpoint::Responses);
    assert_eq!(
        result.from_format,
        ProviderWireFormat::OpenAiChatCompletions
    );
    assert_eq!(result.to_format, ProviderWireFormat::OpenAiResponses);
    serde_json::from_slice(&result.body.expect("translated response body"))
        .expect("translated response is JSON")
}

#[test]
fn response_fixture_maps_content_tool_arguments_and_usage() {
    assert_eq!(
        translate(
            json!({
                "id": "chatcmpl_test",
                "model": "compat-model",
                "created": 1_700_000_000u64,
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": [{"text": "hello"}, {"content": "world"}, {"text": ""}],
                        "tool_calls": [{
                            "id": "call_test",
                            "function": {
                                "name": "functions.exec_command",
                                "arguments": r#"{"cmd":"ls"}"#
                            }
                        }]
                    }
                }],
                "usage": {"prompt_tokens": 3, "completion_tokens": 4}
            }),
            1_700_000_123,
        ),
        json!({
            "id": "chatcmpl_test",
            "object": "response",
            "created_at": 1_700_000_000u64,
            "model": "compat-model",
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {"type": "output_text", "text": "hello"},
                        {"type": "output_text", "text": "world"}
                    ]
                },
                {
                    "type": "function_call",
                    "call_id": "call_test",
                    "name": "exec_command",
                    "namespace": "functions",
                    "arguments": r#"{"cmd":"rtk ls"}"#
                }
            ],
            "usage": {"input_tokens": 3, "output_tokens": 4, "total_tokens": 7}
        })
    );
}

#[test]
fn sparse_response_fixture_keeps_response_defaults() {
    assert_eq!(
        translate(json!({"choices": []}), 1_700_000_123),
        json!({
            "id": "resp_prodex",
            "object": "response",
            "created_at": 1_700_000_123u64,
            "model": "unknown",
            "output": []
        })
    );
}
