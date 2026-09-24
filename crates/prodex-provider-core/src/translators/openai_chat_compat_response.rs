#[path = "openai_chat_compat_response/stream.rs"]
mod stream;

pub(crate) use self::stream::translate_chat_stream_event_to_responses;
#[cfg(any(not(feature = "mojo"), test))]
use super::openai_chat_compat_util::{
    chat_response_body_rust, chat_usage_to_responses_usage_rust,
    message_content_to_output_content_rust, rtk_wrapped_tool_arguments_rust,
    split_flat_namespace_tool_name_rust, stringify_arguments,
};
use super::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformResult,
    ProviderWireFormat, Value,
};
#[cfg(feature = "mojo")]
use crate::mojo_json::Document;
#[cfg(any(not(feature = "mojo"), test))]
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn translate_chat_response_to_responses(
    provider: ProviderId,
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    translate_chat_response_to_responses_at(provider, input, unix_now_secs())
}

fn translate_chat_response_to_responses_at(
    provider: ProviderId,
    input: ProviderTransformInput,
    now_secs: u64,
) -> ProviderTransformResult {
    if input.endpoint != ProviderEndpoint::Responses {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            format!(
                "{} translator only translates responses responses",
                provider.label()
            ),
        );
    }

    let value: Value = match serde_json::from_slice(&input.body) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                format!("failed to parse chat completions response JSON: {error}"),
            );
        }
    };

    #[cfg(feature = "mojo")]
    let body = {
        let mut document = Document::default();
        document.openai_chat_context(&value, Some(now_secs));
        let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
        prodex_mojo_core::json::transform_openai_chat_response(&document.nodes, raw)
            .unwrap_or_else(|error| panic!("Mojo OpenAI chat response transform failed: {error:?}"))
    };
    #[cfg(not(feature = "mojo"))]
    let body = translate_chat_response_body_rust(&value, now_secs);

    ProviderTransformResult::lossless(
        provider,
        input.endpoint,
        ProviderWireFormat::OpenAiChatCompletions,
        ProviderWireFormat::OpenAiResponses,
        body,
    )
}

#[cfg(any(not(feature = "mojo"), test))]
fn translate_chat_response_body_rust(value: &Value, now_secs: u64) -> Vec<u8> {
    let mut output = Vec::new();
    if let Some(message) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
    {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("assistant");
        let content_items = message_content_to_output_content_rust(message.get("content"));
        if !content_items.is_empty() {
            output.push(json!({
                "type": "message",
                "role": role,
                "content": content_items,
            }));
        }
        if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                let Some(function) = tool_call.get("function") else {
                    continue;
                };
                let Some(flat_name) = function.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let arguments = function
                    .get("arguments")
                    .map(stringify_arguments)
                    .unwrap_or_else(|| "{}".to_string());
                let (namespace, name) = split_flat_namespace_tool_name_rust(flat_name);
                let mut item = json!({
                    "type": "function_call",
                    "call_id": tool_call
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("call_prodex"),
                    "name": name,
                    "arguments": rtk_wrapped_tool_arguments_rust(flat_name, &arguments),
                });
                if let Some(namespace) = namespace {
                    item["namespace"] = Value::String(namespace);
                }
                output.push(item);
            }
        }
    }

    let response_id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("resp_prodex");
    let created_at = value
        .get("created")
        .and_then(Value::as_u64)
        .unwrap_or(now_secs);
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let usage = chat_usage_to_responses_usage_rust(value.get("usage"));
    chat_response_body_rust(response_id, created_at, model, &output, usage.as_ref())
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(all(test, feature = "mojo"))]
#[path = "openai_chat_compat_response/mojo_tests.rs"]
mod mojo_tests;

#[cfg(test)]
mod tests {
    use super::{
        ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformResult,
        ProviderWireFormat, Value, translate_chat_response_to_responses,
        translate_chat_stream_event_to_responses,
    };
    use crate::ProviderTransformLoss;
    use serde_json::json;

    fn response(value: Value) -> ProviderTransformResult {
        translate_chat_response_to_responses(
            ProviderId::Anthropic,
            ProviderTransformInput::new(
                ProviderEndpoint::Responses,
                serde_json::to_vec(&value).unwrap(),
            ),
        )
    }

    fn stream(event: &str) -> ProviderTransformResult {
        translate_chat_stream_event_to_responses(
            ProviderId::Anthropic,
            ProviderTransformInput::new(ProviderEndpoint::Responses, event.as_bytes()),
        )
    }

    fn event_data(result: ProviderTransformResult) -> Value {
        let body = String::from_utf8(result.body.unwrap()).unwrap();
        serde_json::from_str(body.lines().nth(1).unwrap().strip_prefix("data: ").unwrap()).unwrap()
    }

    #[test]
    fn response_parity_maps_text_tool_arguments_and_usage() {
        let result = response(json!({
            "id": "chatcmpl_test",
            "model": "compat-model",
            "created": 1700000000u64,
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": [{"text": "hello"}, {"content": "world"}, {"text": ""}],
                    "tool_calls": [{
                        "id": "call_test",
                        "function": {
                            "name": "functions.exec_command",
                            "arguments": "{\"cmd\":\"ls\"}"
                        }
                    }]
                }
            }],
            "usage": {"prompt_tokens": 3, "completion_tokens": 4}
        }));

        assert!(matches!(result.loss, ProviderTransformLoss::Lossless));
        let body: Value = serde_json::from_slice(&result.body.unwrap()).unwrap();
        assert_eq!(body["output"][0]["content"][1]["text"], "world");
        assert_eq!(body["output"][1]["namespace"], "functions");
        assert_eq!(body["output"][1]["name"], "exec_command");
        assert_eq!(body["output"][1]["arguments"], r#"{"cmd":"rtk ls"}"#);
        assert_eq!(body["usage"]["total_tokens"], 7);
    }

    #[test]
    fn response_parity_keeps_defaults_for_sparse_chat_response() {
        let result = response(json!({"choices": []}));
        let body: Value = serde_json::from_slice(&result.body.unwrap()).unwrap();

        assert_eq!(body["id"], "resp_prodex");
        assert_eq!(body["model"], "unknown");
        assert!(body["created_at"].as_u64().is_some());
        assert_eq!(body["output"], json!([]));
        assert!(body.get("usage").is_none());
    }

    #[test]
    fn stream_parity_prefers_tool_text_then_finish() {
        let tool = stream(
            "data: {\"choices\":[{\"delta\":{\"content\":\"ignored\",\"tool_calls\":[{\"id\":\"call_test\",\"function\":{\"name\":\"functions.exec_command\",\"arguments\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}}]}}]}\n\n",
        );
        assert!(matches!(tool.loss, ProviderTransformLoss::Lossless));
        assert_eq!(
            event_data(tool),
            json!({
                "call_id": "call_test",
                "delta": r#"{"cmd":"rtk ls"}"#,
                "type": "response.function_call_arguments.delta"
            })
        );

        let text = stream("data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n");
        assert_eq!(
            event_data(text),
            json!({"type": "response.output_text.delta", "delta": "hello"})
        );

        let done = stream("data: {\"choices\":[{\"finish_reason\":\"stop\"}]}\n\n");
        assert_eq!(event_data(done), json!({}));
    }

    #[test]
    fn stream_parity_rejects_unrecognized_chat_event() {
        let result = stream("data: {\"choices\":[{\"delta\":{}}]}\n\n");
        assert!(matches!(
            result.loss,
            ProviderTransformLoss::UnsupportedUpstream { .. }
        ));
        assert_eq!(
            result.from_format,
            ProviderWireFormat::OpenAiChatCompletions
        );
    }
}
