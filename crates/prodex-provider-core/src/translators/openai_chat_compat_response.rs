#[path = "openai_chat_compat_response/stream.rs"]
mod stream;

pub(crate) use self::stream::translate_chat_stream_event_to_responses;
use super::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformResult,
    ProviderWireFormat, Value, chat_usage_to_responses_usage, json,
    message_content_to_output_content, rtk_wrapped_tool_arguments, split_flat_namespace_tool_name,
    stringify_arguments,
};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn translate_chat_response_to_responses(
    provider: ProviderId,
    input: ProviderTransformInput,
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

        let content_items = message_content_to_output_content(message.get("content"));
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
                let (namespace, name) = split_flat_namespace_tool_name(flat_name);
                let mut item = json!({
                    "type": "function_call",
                    "call_id": tool_call
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("call_prodex"),
                    "name": name,
                    "arguments": rtk_wrapped_tool_arguments(flat_name, &arguments),
                });
                if let Some(namespace) = namespace {
                    item["namespace"] = Value::String(namespace);
                }
                output.push(item);
            }
        }
    }

    let mut response = json!({
        "id": value.get("id").and_then(Value::as_str).unwrap_or("resp_prodex"),
        "object": "response",
        "created_at": value
            .get("created")
            .and_then(Value::as_u64)
            .unwrap_or_else(unix_now_secs),
        "model": value.get("model").and_then(Value::as_str).unwrap_or("unknown"),
        "output": output,
    });

    if let Some(usage) = chat_usage_to_responses_usage(value.get("usage")) {
        response["usage"] = usage;
    }

    ProviderTransformResult::lossless(
        provider,
        input.endpoint,
        ProviderWireFormat::OpenAiChatCompletions,
        ProviderWireFormat::OpenAiResponses,
        serde_json::to_vec(&response).expect("chat compatibility response serializes"),
    )
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
