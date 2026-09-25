//! DeepSeek SSE stream event translation.

use super::{deepseek_passthrough_endpoint, deepseek_stream_event_from_chat_value};
use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde::Deserialize;
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

fn deserialize_usize_or_default<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_default())
}

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(value.as_str().map(str::to_string))
}

fn deserialize_value_vec<'de, D>(deserializer: D) -> Result<Vec<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(value.as_array().cloned().unwrap_or_default())
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct DeepSeekProviderCoreStreamToolCallDelta {
    #[serde(default, deserialize_with = "deserialize_usize_or_default")]
    pub index: usize,
    #[serde(
        default,
        rename = "id",
        deserialize_with = "deserialize_optional_string"
    )]
    pub call_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub name: Option<String>,
    #[serde(
        default,
        rename = "arguments",
        deserialize_with = "deserialize_optional_string"
    )]
    pub argument_delta: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub thought_signature: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeepSeekProviderCoreStreamChunkMetadata {
    pub model: Option<String>,
    pub created_at: Option<u64>,
    pub system_fingerprint: Option<String>,
    pub usage: Option<Value>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct DeepSeekProviderCoreStreamChoiceMetadata {
    #[serde(default)]
    pub logprobs: Option<Value>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub finish_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct DeepSeekProviderCoreStreamChoiceDelta {
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub reasoning_content: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub refusal: Option<String>,
    #[serde(default, deserialize_with = "deserialize_value_vec")]
    pub annotations: Vec<Value>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub content: Option<String>,
    #[serde(default, deserialize_with = "deserialize_value_vec")]
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
    let Some(body) = deepseek_stream_event_from_chat_value(&value) else {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            "DeepSeek SSE event does not contain a supported text or function-call delta",
        );
    };
    ProviderTransformResult::lossless(
        provider,
        input.endpoint,
        ProviderWireFormat::OpenAiChatCompletions,
        ProviderWireFormat::OpenAiResponses,
        body,
    )
}

#[cfg(test)]
#[path = "stream/mojo_tests.rs"]
mod mojo_tests;
