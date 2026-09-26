#[cfg(feature = "mojo")]
use super::super::openai_chat_compat::translate_responses_request_to_chat;
#[cfg(feature = "mojo")]
use super::anthropic_mojo_value;
#[cfg(feature = "mojo")]
use crate::ProviderTransformLoss;
use crate::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformResult,
    ProviderWireFormat,
};
#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};
#[cfg(feature = "mojo")]
use serde_json::Map;
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
#[cfg(feature = "mojo")]
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "mojo")]
#[path = "messages/mojo_request.rs"]
mod mojo_request;
#[cfg(feature = "mojo")]
#[path = "messages/response.rs"]
mod response;
#[path = "messages/stream.rs"]
mod stream;
#[path = "messages/web_search.rs"]
mod web_search;

pub(super) use stream::translate_anthropic_stream_event_to_responses;
#[cfg(feature = "mojo")]
use web_search::anthropic_web_search_call;
#[cfg(feature = "mojo")]
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

#[cfg(not(feature = "mojo"))]
pub(super) fn translate_responses_request_to_anthropic(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    unsupported(
        input.endpoint,
        "Anthropic Messages request translation requires Mojo support",
    )
}

#[cfg(feature = "mojo")]
pub(super) fn translate_responses_request_to_anthropic(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if input.endpoint != ProviderEndpoint::Responses {
        return unsupported(
            input.endpoint,
            "native Messages translation only supports responses",
        );
    }

    let source: Value = match serde_json::from_slice(&input.body) {
        Ok(value) => value,
        Err(error) => return rejected(format!("failed to parse Responses request JSON: {error}")),
    };
    let Some(source_object) = source.as_object() else {
        return rejected("Responses request body must be a JSON object");
    };
    if let Some(field) = ["presence_penalty", "frequency_penalty", "seed", "user"]
        .into_iter()
        .find(|field| source_object.contains_key(*field))
    {
        return rejected(format!(
            "Anthropic Messages does not translate Responses `{field}`"
        ));
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

#[cfg(not(feature = "mojo"))]
pub(super) fn translate_chat_request_to_anthropic(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    ProviderTransformResult::unsupported(
        ProviderId::Anthropic,
        input.endpoint,
        ProviderWireFormat::OpenAiChatCompletions,
        ProviderWireFormat::AnthropicMessages,
        "Anthropic Messages request translation requires Mojo support",
    )
}

#[cfg(feature = "mojo")]
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
    match mojo_request::transform(&chat) {
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

#[cfg(not(feature = "mojo"))]
pub(super) fn translate_anthropic_response_to_responses(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    ProviderTransformResult::unsupported(
        ProviderId::Anthropic,
        input.endpoint,
        ProviderWireFormat::AnthropicMessages,
        ProviderWireFormat::OpenAiResponses,
        "Anthropic Messages response translation requires Mojo support",
    )
}

#[cfg(feature = "mojo")]
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

#[cfg(feature = "mojo")]
fn anthropic_response_envelope_mojo(
    value: &Value,
    output: Vec<Value>,
    created_at: u64,
) -> Result<Value, String> {
    let id = value.get("id").map(json_fragment).transpose()?;
    let model = value.get("model").map(json_fragment).transpose()?;
    let output = json_fragment(&Value::Array(output))?;
    let usage = value.get("usage").and_then(Value::as_object);
    let stop_reason = value.get("stop_reason").map(json_fragment).transpose()?;
    let mut flags = i64::from(usage.is_some());
    let web_search_requests = usage
        .and_then(|usage| usage.get("server_tool_use"))
        .and_then(|usage| usage.get("web_search_requests"))
        .and_then(Value::as_u64);
    if web_search_requests.is_some() {
        flags |= 2;
    }
    if stop_reason.is_some() {
        flags |= 4;
    }
    let output_tokens = json_fragment(&Value::from(
        usage
            .and_then(|usage| usage.get("output_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
    ))?;
    let web_search_requests = web_search_requests
        .map(Value::from)
        .map(|value| json_fragment(&value))
        .transpose()?;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ResponseEnvelope);
    input.id = id.as_deref();
    input.model = model.as_deref();
    input.blocks = Some(&output);
    input.created_at = created_at;
    input.choice_kind = flags;
    input.count = usage
        .and_then(|usage| usage.get("input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    input.max_tokens = Some(&output_tokens);
    input.arguments = stop_reason.as_deref();
    input.tool_use_id = web_search_requests.as_deref();
    anthropic_mojo_value(input)
}

#[cfg(feature = "mojo")]
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

#[cfg(feature = "mojo")]
fn remap_result(mut result: ProviderTransformResult) -> ProviderTransformResult {
    result.to_format = ProviderWireFormat::AnthropicMessages;
    result
}

#[cfg(feature = "mojo")]
fn rejected(reason: impl Into<String>) -> ProviderTransformResult {
    ProviderTransformResult::rejected(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::AnthropicMessages,
        reason,
    )
}

#[cfg(feature = "mojo")]
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
