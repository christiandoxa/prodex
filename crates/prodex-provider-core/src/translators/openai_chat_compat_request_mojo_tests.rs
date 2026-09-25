use super::*;
use crate::translator::ProviderTransformLoss;
use serde_json::{Value, json};

enum Expected {
    Body(Value),
    Rejected(&'static str),
    InvalidJson,
}

fn assert_fixture(body: &[u8], input_model: Option<&str>, expected: Expected) {
    let mut input = ProviderTransformInput::new(ProviderEndpoint::Responses, body.to_vec());
    input.model = input_model.map(str::to_owned);
    let result = translate_responses_request_to_chat(ProviderId::Anthropic, input, "default-model");

    assert_eq!(result.provider, ProviderId::Anthropic);
    assert_eq!(result.endpoint, ProviderEndpoint::Responses);
    assert_eq!(result.from_format, ProviderWireFormat::OpenAiResponses);
    assert_eq!(result.to_format, ProviderWireFormat::OpenAiChatCompletions);
    assert!(result.headers.is_empty());
    assert!(result.metadata.is_empty());

    match expected {
        Expected::Body(expected) => {
            assert!(matches!(&result.loss, ProviderTransformLoss::Lossless));
            let Some(body) = result.body.as_deref() else {
                panic!("expected translated body");
            };
            let actual: Value = serde_json::from_slice(body).expect("translated body is JSON");
            assert_eq!(actual, expected);
        }
        Expected::Rejected(expected) => {
            assert!(result.body.is_none());
            let ProviderTransformLoss::Rejected { reason } = &result.loss else {
                panic!("expected rejected request");
            };
            assert_eq!(reason, expected);
        }
        Expected::InvalidJson => {
            assert!(result.body.is_none());
            let ProviderTransformLoss::Rejected { reason } = &result.loss else {
                panic!("expected invalid JSON rejection");
            };
            assert!(reason.starts_with("failed to parse Responses request JSON:"));
        }
    }
}

fn assert_value_fixture(request: Value, input_model: Option<&str>, expected: Expected) {
    assert_fixture(
        &serde_json::to_vec(&request).expect("fixture serializes"),
        input_model,
        expected,
    );
}

#[test]
fn openai_chat_request_matches_translation_fixtures() {
    assert_value_fixture(
        json!({
            "model": "request-model",
            "instructions": "system",
            "input": [
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "héllo 東京"}]},
                {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "done"}]}
            ],
            "max_output_tokens": 17,
            "stream": true
        }),
        None,
        Expected::Body(json!({
            "model": "request-model",
            "messages": [
                {"role": "system", "content": "system"},
                {"role": "user", "content": "héllo 東京"},
                {"role": "assistant", "content": "done"}
            ],
            "max_tokens": 17,
            "stream": true
        })),
    );
    assert_value_fixture(
        json!({
            "input": "hello",
            "temperature": 0.2,
            "top_p": 0.9,
            "presence_penalty": 0.1,
            "frequency_penalty": 0.3,
            "seed": 42,
            "max_completion_tokens": 77,
            "max_output_tokens": 88,
            "max_tokens": 99,
            "tools": [{"type": "function", "name": "f"}],
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "user": "u",
            "stream": true
        }),
        None,
        Expected::Body(json!({
            "model": "default-model",
            "messages": [{"role": "user", "content": "hello"}],
            "max_tokens": 77,
            "stream": true,
            "temperature": 0.2,
            "top_p": 0.9,
            "presence_penalty": 0.1,
            "frequency_penalty": 0.3,
            "seed": 42,
            "tools": [{"type": "function", "name": "f"}],
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "user": "u"
        })),
    );
    assert_value_fixture(
        json!({"input": [
            {"type": "function_call", "call_id": "c1", "namespace": "agents", "name": "run", "arguments": {"x": 1, "z": 2}},
            {"type": "function_call_output", "call_id": "c1", "output": {"ok": true}}
        ]}),
        None,
        Expected::Body(json!({
            "model": "default-model",
            "messages": [
                {"role": "assistant", "content": "", "tool_calls": [{
                    "id": "c1",
                    "type": "function",
                    "function": {"name": "agents.run", "arguments": "{\"x\":1,\"z\":2}"}
                }]},
                {"role": "tool", "tool_call_id": "c1", "content": "{\"ok\":true}"}
            ],
            "stream": false
        })),
    );
    assert_value_fixture(
        json!({
            "input": "hello",
            "instructions": 17,
            "model": 42,
            "stream": "true",
            "temperature": "warm",
            "max_output_tokens": false,
            "tools": "raw-tools"
        }),
        Some("caller-model"),
        Expected::Body(json!({
            "model": "caller-model",
            "messages": [{"role": "user", "content": "hello"}],
            "max_tokens": false,
            "stream": false,
            "temperature": "warm",
            "tools": "raw-tools"
        })),
    );
    assert_value_fixture(
        json!({"input": [{"type": null, "content": "wrong-type but text"}]}),
        None,
        Expected::Body(json!({
            "model": "default-model",
            "messages": [{"role": "user", "content": "wrong-type but text"}],
            "stream": false
        })),
    );
    assert_value_fixture(
        json!({"instructions": "", "input": " "}),
        None,
        Expected::Body(json!({
            "model": "default-model",
            "messages": [{"role": "user", "content": " "}],
            "stream": false
        })),
    );
    assert_value_fixture(
        json!({"input": [{"type": "input_text", "text": " \t "}, {"type": "output_text", "text": "assistant"}]}),
        None,
        Expected::Body(json!({
            "model": "default-model",
            "messages": [{"role": "user", "content": " \t \nassistant"}],
            "stream": false
        })),
    );
    assert_value_fixture(
        json!({"n": 2.0, "input": "hello"}),
        None,
        Expected::Body(json!({
            "model": "default-model",
            "messages": [{"role": "user", "content": "hello"}],
            "stream": false
        })),
    );
}

#[test]
fn openai_chat_request_rejects_unmapped_inputs_in_precedence_order() {
    let rejected = [
        (
            json!({"messages": [], "response_format": {}, "reasoning": {}, "input": "x"}),
            "anthropic Responses chat-compat expects Responses input, not raw chat-completions messages",
        ),
        (
            json!({"response_format": {}, "reasoning": {}, "input": "x"}),
            "anthropic Responses chat-compat does not translate response_format controls",
        ),
        (
            json!({"reasoning": {}, "previous_response_id": "resp_1", "input": "x"}),
            "anthropic Responses chat-compat does not map Responses reasoning controls",
        ),
        (
            json!({"previous_response_id": "resp_1", "input": "x"}),
            "anthropic Responses chat-compat does not map previous_response_id continuation state",
        ),
        (
            json!({"text": {"format": {}}, "n": 2, "input": "x"}),
            "anthropic Responses chat-compat does not translate text.format controls",
        ),
        (
            json!({"n": 2, "input": "x"}),
            "anthropic Responses chat-compat returns only the first choice and does not support n>1",
        ),
        (
            json!({"metadata": {}, "safety_identifier": "s", "web_search_options": {}, "input": "x"}),
            "anthropic Responses chat-compat does not translate request metadata",
        ),
        (
            json!({"safety_identifier": "s", "web_search_options": {}, "input": "x"}),
            "anthropic Responses chat-compat does not translate safety_identifier",
        ),
        (
            json!({"web_search_options": {}, "input": "x"}),
            "anthropic Responses chat-compat does not translate web_search_options",
        ),
        (
            json!({"tools": [{"type": "custom", "name": "x"}], "tool_choice": {"type": "mcp"}, "input": "x"}),
            "anthropic Responses chat-compat only forwards function tools",
        ),
        (
            json!({"tool_choice": {"type": "mcp", "name": "x"}, "parallel_tool_calls": false, "input": "x"}),
            "anthropic Responses chat-compat only forwards function tool_choice controls",
        ),
        (
            json!({"parallel_tool_calls": false, "logprobs": true, "input": "x"}),
            "anthropic Responses chat-compat does not prove a compatible parallel_tool_calls=false control",
        ),
        (
            json!({"logprobs": true, "top_logprobs": 2, "input": "x"}),
            "anthropic Responses chat-compat does not translate logprobs controls",
        ),
        (
            json!({"stop_sequences": ["x"], "input": "x"}),
            "anthropic Responses chat-compat does not translate stop_sequences",
        ),
        (
            json!({"input": [{"type": "custom_tool_call", "name": "x"}, {"type": "input_image", "image_url": "data:image/png;base64,AA=="}]}),
            "anthropic Responses chat-compat only translates message/function-call history items",
        ),
        (
            json!({"input": [{"type": "input_image", "image_url": "data:image/png;base64,AA=="}]}),
            "anthropic Responses chat-compat currently translates only text input content",
        ),
        (
            json!({"input": 42}),
            "Responses request must include a textual input or messages array",
        ),
        (
            json!({"input": []}),
            "Responses request must include a textual input or messages array",
        ),
        (
            json!({"input": null}),
            "Responses request must include a textual input or messages array",
        ),
        (
            json!({"input": [{"type": "function_call", "name": null, "tool_name": "blocked", "arguments": "{}"}]}),
            "Responses request must include a textual input or messages array",
        ),
        (
            json!({"input": [{"type": "function_call_output", "call_id": 17, "id": "blocked", "output": "x"}]}),
            "Responses request must include a textual input or messages array",
        ),
    ];
    for (request, reason) in rejected {
        assert_value_fixture(request, None, Expected::Rejected(reason));
    }
}

#[test]
fn openai_chat_request_preserves_duplicate_and_large_input_contracts() {
    assert_fixture(
        br#"{"input":"first","input":"last","model":"first-model","model":"last-model"}"#,
        None,
        Expected::Body(json!({
            "model": "last-model",
            "messages": [{"role": "user", "content": "last"}],
            "stream": false
        })),
    );
    assert_fixture(b"{broken", None, Expected::InvalidJson);
    assert_fixture(
        b"null",
        None,
        Expected::Rejected("Responses request body must be a JSON object"),
    );

    let large_text = "東京🌍".repeat(32_768);
    assert_value_fixture(
        json!({"input": large_text.clone()}),
        None,
        Expected::Body(json!({
            "model": "default-model",
            "messages": [{"role": "user", "content": large_text}],
            "stream": false
        })),
    );
}
