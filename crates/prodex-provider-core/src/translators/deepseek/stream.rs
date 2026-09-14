//! DeepSeek SSE stream event translation.

use super::{deepseek_passthrough_endpoint, deepseek_stream_event_from_chat_value};
use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::Value;

#[path = "stream/response_values.rs"]
mod response_values;
pub use response_values::{
    deepseek_provider_core_stream_chat_assistant_message,
    deepseek_provider_core_stream_response_value,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeepSeekProviderCoreStreamChatToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
    pub thought_signature: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeepSeekProviderCoreStreamToolCallDelta {
    pub index: usize,
    pub call_id: Option<String>,
    pub name: Option<String>,
    pub argument_delta: Option<String>,
    pub thought_signature: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeepSeekProviderCoreStreamChunkMetadata {
    pub model: Option<String>,
    pub created_at: Option<u64>,
    pub system_fingerprint: Option<String>,
    pub usage: Option<Value>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeepSeekProviderCoreStreamChoiceMetadata {
    pub logprobs: Option<Value>,
    pub finish_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeepSeekProviderCoreStreamChoiceDelta {
    pub reasoning_content: Option<String>,
    pub refusal: Option<String>,
    pub annotations: Vec<Value>,
    pub content: Option<String>,
    pub tool_calls: Vec<Value>,
}

#[path = "stream/shaping.rs"]
mod shaping;
pub use shaping::*;

pub(super) fn deepseek_transform_stream_event(
    provider: ProviderId,
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if deepseek_passthrough_endpoint(input.endpoint) {
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            input.body,
        );
    }
    if input.endpoint != ProviderEndpoint::Responses {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            format!(
                "DeepSeek translator does not support {}",
                input.endpoint.label()
            ),
        );
    }
    let event = String::from_utf8_lossy(&input.body);
    let Some(data) = event
        .strip_prefix("data: ")
        .and_then(|s| s.strip_suffix("\n\n"))
    else {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            "DeepSeek SSE event must use data: <json> framing",
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
                format!("failed to parse DeepSeek SSE JSON: {error}"),
            );
        }
    };
    let Some((event_name, transformed)) = deepseek_stream_event_from_chat_value(&value) else {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            "DeepSeek SSE event does not contain a supported text or function-call delta",
        );
    };
    let body = format!("event: {event_name}\ndata: {}\n\n", transformed);
    ProviderTransformResult::lossless(
        provider,
        input.endpoint,
        ProviderWireFormat::OpenAiChatCompletions,
        ProviderWireFormat::OpenAiResponses,
        body.into_bytes(),
    )
}
