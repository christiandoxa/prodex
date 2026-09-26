use super::tooling::deepseek_responses_tool_call_item;
use crate::bridge::{
    provider_core_chat_compatible_created_at, provider_core_chat_compatible_responses_usage,
};
use serde_json::{Value, json};

use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation, deepseek_kernel};

#[path = "response/metadata.rs"]
mod metadata;

pub(super) fn deepseek_stream_event_from_chat_value(value: &Value) -> Option<Vec<u8>> {
    let mut document = crate::mojo_json::Document::default();
    document.openai_chat_context(value, None);
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    prodex_mojo_core::json::transform_deepseek_chat_stream_event(&document.nodes, raw)
        .unwrap_or_else(|error| panic!("Mojo DeepSeek stream event failed: {error:?}"))
}

pub(super) fn deepseek_responses_value_from_chat_value(value: &Value) -> Result<Value, String> {
    let response_id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("chatcmpl_prodex");
    let created_at = value
        .get("created")
        .and_then(Value::as_u64)
        .unwrap_or_else(deepseek_created_at);
    let message = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"));
    let mut output = Vec::new();
    let mut tool_call_error = None;
    if let Some(text) = message
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        output.push(json!({
            "type":"message",
            "role":"assistant",
            "content":[{"type":"output_text","text":text}],
        }));
    }
    if let Some(tool_calls) = message
        .and_then(|message| message.get("tool_calls"))
        .and_then(Value::as_array)
    {
        for tool_call in tool_calls {
            match deepseek_responses_tool_call_item(tool_call) {
                Ok(Some(item)) => output.push(item),
                Ok(None) => {}
                Err(error) => {
                    tool_call_error = Some(error);
                    break;
                }
            }
        }
    }
    let output = serde_json::to_string(&output)
        .map_err(|error| format!("failed to serialize DeepSeek response output: {error}"))?;
    let usage = value
        .get("usage")
        .and_then(deepseek_responses_usage)
        .map(|value| serde_json::to_string(&value))
        .transpose()
        .map_err(|error| format!("failed to serialize DeepSeek response usage: {error}"))?;
    let metadata = metadata::deepseek_response_metadata(value, message)
        .map(|value| serde_json::to_string(&value))
        .transpose()
        .map_err(|error| format!("failed to serialize DeepSeek response metadata: {error}"))?;
    let error_message = tool_call_error.as_deref();
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::BufferedResponse);
    input.response_id = Some(response_id);
    input.created_at = created_at;
    input.model = Some(
        value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("deepseek-chat"),
    );
    input.output = Some(&output);
    input.usage = usage.as_deref();
    input.metadata = metadata.as_deref();
    input.error_code = error_message.map(|_| "invalid_tool_call_arguments");
    input.error_message = error_message;
    let body = deepseek_kernel(input).map_err(|_| {
        "DeepSeek response exceeds the bounded Mojo normalization limit".to_string()
    })?;
    serde_json::from_slice(&body)
        .map_err(|error| format!("DeepSeek Mojo returned invalid response JSON: {error}"))
}

pub(super) fn deepseek_responses_usage(usage: &Value) -> Option<Value> {
    provider_core_chat_compatible_responses_usage(usage, "deepseek")
}

pub(super) fn deepseek_created_at() -> u64 {
    provider_core_chat_compatible_created_at()
}

#[cfg(test)]
mod tests {
    use super::deepseek_responses_value_from_chat_value;
    use serde_json::json;

    #[test]
    fn buffered_response_matches_expected_text_tool_usage_and_metadata() {
        let value = json!({
            "id": "chatcmpl_test_1",
            "model": "deepseek-chat",
            "created": 1700000000,
            "choices": [{
                "message": {
                    "content": "こんにちは 🌋",
                    "reasoning_content": "考えています",
                    "refusal": "拒否",
                    "annotations": [{"type": "citation", "url": "https://example.com/source"}],
                    "tool_calls": [{
                        "id": "call_東京",
                        "function": {
                            "name": "functions.検索",
                            "arguments": "{\"query\":\"café 🍜\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls",
                "logprobs": {"tokens": ["ok"]}
            }],
            "system_fingerprint": "fp_test_1",
            "usage": {
                "prompt_tokens": 11,
                "completion_tokens": 7,
                "total_tokens": 18,
                "prompt_cache_hit_tokens": 5,
                "prompt_cache_miss_tokens": 6,
                "completion_tokens_details": {"reasoning_tokens": 2}
            }
        });

        assert_eq!(
            deepseek_responses_value_from_chat_value(&value).unwrap(),
            json!({
                "id": "chatcmpl_test_1",
                "object": "response",
                "created_at": 1700000000,
                "model": "deepseek-chat",
                "output": [
                    {
                        "type": "message",
                        "role": "assistant",
                        "content": [{"type": "output_text", "text": "こんにちは 🌋"}]
                    },
                    {
                        "type": "function_call",
                        "call_id": "call_東京",
                        "namespace": "functions",
                        "name": "検索",
                        "arguments": "{\"query\":\"café 🍜\"}"
                    }
                ],
                "usage": {
                    "input_tokens": 11,
                    "output_tokens": 7,
                    "total_tokens": 18,
                    "input_tokens_details": {"cached_tokens": 5},
                    "output_tokens_details": {"reasoning_tokens": 2},
                    "metadata": {
                        "deepseek": {
                            "prompt_cache_hit_tokens": 5,
                            "prompt_cache_miss_tokens": 6
                        }
                    }
                },
                "metadata": {
                    "deepseek": {
                        "annotations": [{"type": "citation", "url": "https://example.com/source"}],
                        "finish_reason": "tool_calls",
                        "logprobs": {"tokens": ["ok"]},
                        "reasoning_content": "考えています",
                        "refusal": "拒否",
                        "system_fingerprint": "fp_test_1"
                    }
                }
            })
        );
    }

    #[test]
    fn buffered_response_matches_expected_malformed_tool_error() {
        let value = json!({
            "created": 42,
            "choices": [{
                "message": {
                    "content": "before error",
                    "tool_calls": [{
                        "function": {"name": "lookup", "arguments": "{bad"}
                    }]
                }
            }]
        });

        assert_eq!(
            deepseek_responses_value_from_chat_value(&value).unwrap(),
            json!({
                "id": "chatcmpl_prodex",
                "object": "response",
                "created_at": 42,
                "model": "deepseek-chat",
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "before error"}]
                }],
                "status": "failed",
                "error": {
                    "code": "invalid_tool_call_arguments",
                    "message": "DeepSeek returned malformed JSON arguments for tool call `lookup`: key must be a string at line 1 column 2"
                }
            })
        );
    }

    #[test]
    fn buffered_response_matches_expected_defaults_when_fields_are_missing() {
        let value = json!({"created": 23});

        assert_eq!(
            deepseek_responses_value_from_chat_value(&value).unwrap(),
            json!({
                "id": "chatcmpl_prodex",
                "object": "response",
                "created_at": 23,
                "model": "deepseek-chat",
                "output": []
            })
        );
    }
}
