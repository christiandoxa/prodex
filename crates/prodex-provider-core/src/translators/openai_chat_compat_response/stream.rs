//! Chat-completions SSE to Responses event normalization.

use super::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformResult,
    ProviderWireFormat, Value,
};

#[cfg(any(not(feature = "mojo"), test))]
use super::super::openai_chat_compat_util::rtk_wrapped_tool_arguments_rust;
#[cfg(any(not(feature = "mojo"), test))]
use serde_json::json;

#[cfg(feature = "mojo")]
fn stream_event_body(
    kind: prodex_mojo_core::rich::OpenAiCompatStreamKind,
    call_id: Option<&str>,
    name: Option<&str>,
    delta: Option<&str>,
) -> Vec<u8> {
    prodex_mojo_core::rich::openai_compat_stream_event(
        prodex_mojo_core::rich::OpenAiCompatStreamInput {
            kind,
            call_id,
            name,
            delta,
        },
    )
    .unwrap_or_else(|error| panic!("Mojo OpenAI compatibility stream event failed: {error:?}"))
}

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
        #[cfg(feature = "mojo")]
        let body = stream_event_body(
            prodex_mojo_core::rich::OpenAiCompatStreamKind::Done,
            None,
            None,
            None,
        );
        #[cfg(not(feature = "mojo"))]
        let body = b"event: response.completed\ndata: {}\n\n".to_vec();
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            body,
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

    #[cfg(feature = "mojo")]
    {
        let mut document = crate::mojo_json::Document::default();
        document.openai_chat_context(&value, None);
        let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
        match prodex_mojo_core::json::transform_openai_chat_stream_event(&document.nodes, raw) {
            Ok(Some(body)) => ProviderTransformResult::lossless(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                body,
            ),
            Ok(None) => ProviderTransformResult::unsupported(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                "chat completions SSE event does not contain a supported text delta",
            ),
            Err(error) => panic!("Mojo OpenAI compatibility stream event failed: {error:?}"),
        }
    }

    #[cfg(not(feature = "mojo"))]
    match translate_chat_stream_value_to_responses_rust(&value) {
        Some(body) => ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            body,
        ),
        None => ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            "chat completions SSE event does not contain a supported text delta",
        ),
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn translate_chat_stream_value_to_responses_rust(value: &Value) -> Option<Vec<u8>> {
    let choice = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first());
    let delta = choice.and_then(|choice| choice.get("delta"));
    let tool_call = delta
        .and_then(|delta| delta.get("tool_calls"))
        .and_then(Value::as_array)
        .and_then(|tool_calls| tool_calls.first());
    let arguments = tool_call
        .and_then(|tool_call| tool_call.get("function"))
        .and_then(|function| function.get("arguments"))
        .and_then(Value::as_str);
    let text = delta
        .and_then(|delta| delta.get("content"))
        .and_then(Value::as_str);
    let finished = choice
        .and_then(|choice| choice.get("finish_reason"))
        .is_some_and(|finish_reason| !finish_reason.is_null());

    if let (Some(tool_call), Some(arguments)) = (tool_call, arguments) {
        let mut payload = json!({
            "type": "response.function_call_arguments.delta",
            "delta": rtk_wrapped_tool_arguments_rust(
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
        return Some(
            format!("event: response.function_call_arguments.delta\ndata: {payload}\n\n")
                .into_bytes(),
        );
    }

    if let Some(text) = text {
        return Some(
            format!(
                "event: response.output_text.delta\ndata: {}\n\n",
                json!({"type": "response.output_text.delta", "delta": text})
            )
            .into_bytes(),
        );
    }

    finished.then(|| b"event: response.completed\ndata: {}\n\n".to_vec())
}

#[cfg(all(test, feature = "mojo"))]
#[path = "stream/mojo_tests.rs"]
mod mojo_tests;
