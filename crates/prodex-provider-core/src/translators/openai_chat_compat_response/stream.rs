//! Chat-completions SSE to Responses event normalization.

use super::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformResult,
    ProviderWireFormat, Value, json, rtk_wrapped_tool_arguments,
};

pub(crate) fn translate_chat_stream_event_to_responses(
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
                "{} translator only translates responses stream events",
                provider.label()
            ),
        );
    }

    let event = String::from_utf8_lossy(&input.body);
    let Some(data) = event
        .strip_prefix("data: ")
        .and_then(|body| body.strip_suffix("\n\n"))
    else {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            "chat completions SSE event must use data: <json> framing",
        );
    };

    if data == "[DONE]" {
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            b"event: response.completed\ndata: {}\n\n".to_vec(),
        );
    }

    let value: Value = match serde_json::from_str(data) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                format!("failed to parse chat completions SSE JSON: {error}"),
            );
        }
    };

    if let Some(tool_call) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("tool_calls"))
        .and_then(Value::as_array)
        .and_then(|tool_calls| tool_calls.first())
        && let Some(arguments) = tool_call
            .get("function")
            .and_then(|function| function.get("arguments"))
            .and_then(Value::as_str)
    {
        let mut payload = json!({
            "type": "response.function_call_arguments.delta",
            "delta": rtk_wrapped_tool_arguments(
                tool_call
                    .get("function")
                    .and_then(|function| function.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                arguments,
            ),
        });
        if let Some(call_id) = tool_call.get("id").and_then(Value::as_str) {
            payload["call_id"] = Value::String(call_id.to_string());
        }
        let body = format!(
            "event: response.function_call_arguments.delta\ndata: {}\n\n",
            payload
        );
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            body.into_bytes(),
        );
    }

    if let Some(text) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("content"))
        .and_then(Value::as_str)
    {
        let body = format!(
            "event: response.output_text.delta\ndata: {}\n\n",
            json!({"type": "response.output_text.delta", "delta": text})
        );
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            body.into_bytes(),
        );
    }

    if value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("finish_reason"))
        .filter(|finish_reason| !finish_reason.is_null())
        .is_some()
    {
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            b"event: response.completed\ndata: {}\n\n".to_vec(),
        );
    }

    ProviderTransformResult::unsupported(
        provider,
        input.endpoint,
        ProviderWireFormat::OpenAiChatCompletions,
        ProviderWireFormat::OpenAiResponses,
        "chat completions SSE event does not contain a supported text delta",
    )
}
