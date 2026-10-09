use super::super::openai_chat_compat::translate_responses_request_to_chat;
use super::anthropic_mojo_value;
use crate::ProviderTransformLoss;
use crate::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformResult,
    ProviderWireFormat,
};
use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};
use serde_json::Map;
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "messages/mojo_request.rs"]
mod mojo_request;
#[path = "messages/response.rs"]
mod response;
#[path = "messages/stream.rs"]
mod stream;
#[path = "messages/web_search.rs"]
mod web_search;

pub(super) use stream::translate_anthropic_stream_event_to_responses;
use web_search::anthropic_web_search_call;
use web_search::merge_anthropic_web_search_result;

pub(super) fn anthropic_web_search_result_sources(block: &Value) -> Result<Vec<Value>, String> {
    web_search::anthropic_web_search_result_sources(block)
}

pub(super) fn anthropic_web_search_stream_item(
    id: &str,
    input_json: &str,
    sources: &[Value],
    in_progress: bool,
) -> Result<Value, String> {
    web_search::anthropic_web_search_stream_item(id, input_json, sources, in_progress)
}

pub(super) fn translate_responses_request_to_anthropic(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if input.endpoint != ProviderEndpoint::Responses {
        return unsupported(
            input.endpoint,
            "native Messages translation only supports responses",
        );
    }

    let chat = translate_responses_request_to_chat(ProviderId::Anthropic, input, "auto");
    if !matches!(chat.loss, ProviderTransformLoss::Lossless) {
        return remap_result(chat);
    }
    let Some(chat_body) = chat.body else {
        return rejected("Responses request translation produced no body");
    };
    let mut result = translate_chat_request_to_anthropic(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        chat_body,
    ));
    result.from_format = ProviderWireFormat::OpenAiResponses;
    result
}

pub(super) fn translate_chat_request_to_anthropic(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if input.endpoint != ProviderEndpoint::Responses {
        return ProviderTransformResult::unsupported(
            ProviderId::Anthropic,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::AnthropicMessages,
            "native Messages translation only supports responses",
        );
    }
    let chat: Value = match serde_json::from_slice(&input.body) {
        Ok(value) => value,
        Err(error) => {
            return rejected_chat(format!("failed to parse translated request JSON: {error}"));
        }
    };
    let transform = match mojo_request::transform(&chat) {
        Ok(transform) => transform,
        Err(reason) => return rejected_chat(reason),
    };
    match transform {
        prodex_mojo_core::json::AnthropicChatRequestTransform::Body(body) => {
            ProviderTransformResult::lossless(
                ProviderId::Anthropic,
                ProviderEndpoint::Responses,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::AnthropicMessages,
                body,
            )
        }
        prodex_mojo_core::json::AnthropicChatRequestTransform::Degraded { body, context_size } => {
            let mut details = BTreeMap::new();
            details.insert(
                "web_search_options.search_context_size".to_string(),
                serde_json::json!({"from": context_size, "to": "provider_default"}),
            );
            ProviderTransformResult::degraded(
                ProviderId::Anthropic,
                ProviderEndpoint::Responses,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::AnthropicMessages,
                body,
                "Anthropic Messages uses the provider default web-search context size",
                details,
            )
        }
        prodex_mojo_core::json::AnthropicChatRequestTransform::Rejected(reason) => {
            rejected_chat(reason)
        }
    }
}

fn json_fragment(value: &Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("Anthropic JSON fragment failed: {error}"))
}

pub(super) fn translate_anthropic_response_to_responses(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if input.endpoint != ProviderEndpoint::Responses {
        return unsupported(
            input.endpoint,
            "native Messages translation only supports responses",
        );
    }
    let value: Value = match serde_json::from_slice(&input.body) {
        Ok(value) => value,
        Err(error) => {
            return rejected_response(format!(
                "failed to parse Anthropic Messages response JSON: {error}"
            ));
        }
    };
    let Some(content) = value.get("content").and_then(Value::as_array) else {
        return rejected_response("Anthropic Messages response must contain a content array");
    };
    let output = match response::anthropic_response_output(content) {
        Ok(output) => output,
        Err(reason) => return rejected_response(reason),
    };

    let response = match anthropic_response_envelope_mojo(&value, output, unix_now_secs()) {
        Ok(response) => response,
        Err(reason) => return rejected_response(reason),
    };

    ProviderTransformResult::lossless(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        ProviderWireFormat::AnthropicMessages,
        ProviderWireFormat::OpenAiResponses,
        serde_json::to_vec(&response).expect("Responses response serializes"),
    )
}

fn anthropic_response_envelope_mojo(
    value: &Value,
    output: Vec<Value>,
    created_at: u64,
) -> Result<Value, String> {
    let source = json_fragment(value)?;
    let output = json_fragment(&Value::Array(output))?;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ResponseEnvelope);
    input.content = Some(&source);
    input.blocks = Some(&output);
    input.created_at = created_at;
    input.choice_kind = -1;
    anthropic_mojo_value(input)
}

fn anthropic_tool_use_item(block: &Value) -> Result<Value, String> {
    let Some(id) = block.get("id").and_then(Value::as_str) else {
        return Err("Anthropic tool_use block must contain id".to_string());
    };
    let Some(full_name) = block.get("name").and_then(Value::as_str) else {
        return Err("Anthropic tool_use block must contain name".to_string());
    };
    let arguments = serde_json::to_string(block.get("input").unwrap_or(&Value::Object(Map::new())))
        .map_err(|error| format!("Anthropic tool input serializes: {error}"))?;
    let arguments =
        crate::provider_core_chat_compatible_rtk_wrapped_tool_arguments(full_name, &arguments);
    let (namespace, name) = crate::provider_core_split_flat_namespace_tool_name(full_name);
    let id = json_fragment(&Value::String(id.to_string()))?;
    let name = json_fragment(&Value::String(name))?;
    let namespace = namespace
        .map(|namespace| json_fragment(&Value::String(namespace)))
        .transpose()?;
    let arguments = json_fragment(&Value::String(arguments))?;
    let mut input = AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ToolUseItem);
    input.id = Some(&id);
    input.name = Some(&name);
    input.namespace = namespace.as_deref();
    input.arguments = Some(&arguments);
    anthropic_mojo_value(input)
}

fn remap_result(mut result: ProviderTransformResult) -> ProviderTransformResult {
    result.to_format = ProviderWireFormat::AnthropicMessages;
    result
}

fn rejected(reason: impl Into<String>) -> ProviderTransformResult {
    ProviderTransformResult::rejected(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::AnthropicMessages,
        reason,
    )
}

fn rejected_chat(reason: impl Into<String>) -> ProviderTransformResult {
    ProviderTransformResult::rejected(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiChatCompletions,
        ProviderWireFormat::AnthropicMessages,
        reason,
    )
}

fn rejected_response(reason: impl Into<String>) -> ProviderTransformResult {
    ProviderTransformResult::rejected(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        ProviderWireFormat::AnthropicMessages,
        ProviderWireFormat::OpenAiResponses,
        reason,
    )
}

fn rejected_stream(reason: impl Into<String>) -> ProviderTransformResult {
    rejected_response(reason)
}

fn empty_lossless_stream() -> ProviderTransformResult {
    ProviderTransformResult::lossless(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        ProviderWireFormat::AnthropicMessages,
        ProviderWireFormat::OpenAiResponses,
        Vec::new(),
    )
}

fn unsupported(endpoint: ProviderEndpoint, reason: impl Into<String>) -> ProviderTransformResult {
    ProviderTransformResult::unsupported(
        ProviderId::Anthropic,
        endpoint,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::AnthropicMessages,
        reason,
    )
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
