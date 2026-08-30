use super::DeepSeekProviderCoreStreamChatToolCall;
use serde_json::Value;

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation};

pub fn deepseek_provider_core_stream_response_value(
    response_id: &str,
    output: Vec<Value>,
    model: Option<&str>,
    usage: Option<Value>,
    provider_metadata: Option<Value>,
    response_metadata: Option<Value>,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut response = serde_json::json!({
            "id": response_id,
            "output": output,
        });
        if let Some(metadata) = provider_metadata {
            response["metadata"] = metadata;
        }
        if let Some(usage) = usage {
            response["usage"] = usage;
        }
        crate::deepseek_bridge::deepseek_provider_core_merge_response_metadata(
            &mut response,
            response_metadata,
        );
        let output =
            serde_json::to_string(&response["output"]).expect("DeepSeek stream output serializes");
        let usage = response
            .get("usage")
            .map(|value| serde_json::to_string(value).expect("DeepSeek usage serializes"));
        let metadata = response
            .get("metadata")
            .map(|value| serde_json::to_string(value).expect("DeepSeek metadata serializes"));
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::StreamResponseValue);
        input.response_id = Some(response_id);
        input.output = Some(&output);
        input.model = model;
        input.usage = usage.as_deref();
        input.metadata = metadata.as_deref();
        return super::super::deepseek_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        let mut response = serde_json::json!({
            "id": response_id,
            "output": output,
        });
        if let Some(model) = model {
            response["model"] = Value::String(model.to_string());
        }
        if let Some(usage) = usage {
            response["usage"] = usage;
        }
        if let Some(metadata) = provider_metadata {
            response["metadata"] = metadata;
        }
        crate::deepseek_bridge::deepseek_provider_core_merge_response_metadata(
            &mut response,
            response_metadata,
        );
        response
    }
}

pub fn deepseek_provider_core_stream_chat_assistant_message(
    output_text: &str,
    reasoning_content: &str,
    tool_calls: &[DeepSeekProviderCoreStreamChatToolCall],
) -> Option<Value> {
    #[cfg(feature = "mojo")]
    {
        if output_text.is_empty() && reasoning_content.is_empty() && tool_calls.is_empty() {
            return None;
        }
        let tool_calls = (!tool_calls.is_empty()).then(|| {
            serde_json::to_string(
                &tool_calls
                    .iter()
                    .map(|tool_call| {
                        let mut value = serde_json::json!({
                            "id": tool_call.call_id,
                            "type": "function",
                            "function": {
                                "name": tool_call.name,
                                "arguments": tool_call.arguments,
                            },
                        });
                        if let Some(signature) = tool_call.thought_signature.as_deref() {
                            value["extra_content"] = serde_json::json!({
                                "google": {
                                    "thought_signature": signature,
                                }
                            });
                        }
                        value
                    })
                    .collect::<Vec<_>>(),
            )
            .expect("DeepSeek stream tool calls serialize")
        });
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::StreamAssistantMessage);
        input.content = (!output_text.is_empty()).then_some(output_text);
        input.reasoning_content = (!reasoning_content.is_empty()).then_some(reasoning_content);
        input.tool_calls = tool_calls.as_deref();
        return Some(super::super::deepseek_mojo_value(input));
    }
    #[cfg(not(feature = "mojo"))]
    {
        if output_text.is_empty() && reasoning_content.is_empty() && tool_calls.is_empty() {
            return None;
        }
        let mut assistant = serde_json::json!({
            "role": "assistant",
            "content": if output_text.is_empty() {
                if tool_calls.is_empty() {
                    Value::Null
                } else {
                    Value::String(String::new())
                }
            } else {
                Value::String(output_text.to_string())
            },
        });
        if !reasoning_content.is_empty() {
            assistant["reasoning_content"] = Value::String(reasoning_content.to_string());
        }
        if !tool_calls.is_empty() {
            assistant["tool_calls"] = Value::Array(
                tool_calls
                    .iter()
                    .map(|tool_call| {
                        let mut value = serde_json::json!({
                            "id": tool_call.call_id,
                            "type": "function",
                            "function": {
                                "name": tool_call.name,
                                "arguments": tool_call.arguments,
                            },
                        });
                        if let Some(signature) = tool_call.thought_signature.as_deref() {
                            value["extra_content"] = serde_json::json!({
                                "google": {
                                    "thought_signature": signature,
                                }
                            });
                        }
                        value
                    })
                    .collect(),
            );
        }
        Some(assistant)
    }
}
