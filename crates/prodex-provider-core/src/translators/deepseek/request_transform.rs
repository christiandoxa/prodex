//! DeepSeek request translation.

use super::{deepseek_passthrough_endpoint, request::deepseek_request_body_from_responses};
use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::Value;

pub(super) fn deepseek_transform_request(
    provider: ProviderId,
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if deepseek_passthrough_endpoint(input.endpoint) {
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            input.body,
        );
    }
    if !matches!(
        input.endpoint,
        ProviderEndpoint::Responses | ProviderEndpoint::ResponsesCompact
    ) {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            format!(
                "DeepSeek translator does not support {}",
                input.endpoint.label()
            ),
        );
    }
    let value: Value = match serde_json::from_slice(&input.body) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiResponses,
                ProviderWireFormat::OpenAiChatCompletions,
                format!("failed to parse Responses request JSON: {error}"),
            );
        }
    };
    let Some(obj) = value.as_object() else {
        return ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            "DeepSeek request body must be a JSON object",
        );
    };
    if matches!(
        obj.get("parallel_tool_calls").and_then(Value::as_bool),
        Some(false)
    ) {
        return ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            "DeepSeek does not expose a compatible parallel_tool_calls=false control",
        );
    }
    let (body, degraded) = match deepseek_request_body_from_responses(obj, &value) {
        Ok(result) => result,
        Err(reason) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiResponses,
                ProviderWireFormat::OpenAiChatCompletions,
                reason,
            );
        }
    };
    let result = if let Some(details) = degraded {
        ProviderTransformResult::degraded(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            body,
            "DeepSeek degrades JSON schema output to json_object",
            details,
        )
    } else {
        ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            body,
        )
    };
    let mut metadata = serde_json::Map::new();
    for header in ["x-codex-turn-state", "session_id"] {
        if let Some(value) = input.headers.get(header) {
            metadata.insert(header.to_string(), Value::String(value.clone()));
        }
    }
    if let Some(previous) = obj.get("previous_response_id").and_then(Value::as_str) {
        metadata.insert(
            "previous_response_id".to_string(),
            Value::String(previous.to_string()),
        );
    }
    if metadata.is_empty() {
        result
    } else {
        result.with_metadata("continuation", Value::Object(metadata))
    }
}
