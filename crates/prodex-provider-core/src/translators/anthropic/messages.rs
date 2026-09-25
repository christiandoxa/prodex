use super::super::openai_chat_compat::translate_responses_request_to_chat;
#[cfg(feature = "mojo")]
use super::anthropic_mojo_value;
use crate::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformLoss,
    ProviderTransformResult, ProviderWireFormat,
};
#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};
#[cfg(any(not(feature = "mojo"), test))]
use serde_json::json;
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "mojo")]
#[path = "messages/mojo_request.rs"]
mod mojo_request;
#[cfg(any(not(feature = "mojo"), test))]
#[path = "messages/request_fallback.rs"]
mod request_fallback;
#[cfg(feature = "mojo")]
#[path = "messages/response.rs"]
mod response;
#[path = "messages/stream.rs"]
mod stream;
#[path = "messages/web_search.rs"]
mod web_search;

pub(super) use stream::translate_anthropic_stream_event_to_responses;
use web_search::anthropic_web_search_call;
#[cfg(any(not(feature = "mojo"), test))]
use web_search::anthropic_web_search_tool;
#[cfg(feature = "mojo")]
use web_search::merge_anthropic_web_search_result;

#[cfg(any(not(feature = "mojo"), test))]
use request_fallback::{build_anthropic_chat_request_rust, validate_anthropic_chat_fields};

#[cfg(any(not(feature = "mojo"), test))]
type AnthropicChatRequest = (Map<String, Value>, BTreeMap<String, Value>);

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

#[cfg(any(not(feature = "mojo"), test))]
fn translate_chat_request_to_anthropic_rust(
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
    let Some(chat) = chat.as_object() else {
        return rejected_chat("translated request body must be a JSON object");
    };
    if let Err(reason) = validate_anthropic_chat_fields(chat) {
        return rejected_chat(reason);
    }
    let (system, messages) = match anthropic_messages(chat.get("messages")) {
        Ok(messages) => messages,
        Err(reason) => return rejected_chat(reason),
    };
    let (request, degradation_details) =
        match build_anthropic_chat_request_rust(&system, messages, chat) {
            Ok(request) => request,
            Err(reason) => return rejected_chat(reason),
        };
    let body = serde_json::to_vec(&Value::Object(request)).expect("Anthropic request serializes");
    if degradation_details.is_empty() {
        ProviderTransformResult::lossless(
            ProviderId::Anthropic,
            ProviderEndpoint::Responses,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::AnthropicMessages,
            body,
        )
    } else {
        ProviderTransformResult::degraded(
            ProviderId::Anthropic,
            ProviderEndpoint::Responses,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::AnthropicMessages,
            body,
            "Anthropic Messages uses the provider default web-search context size",
            degradation_details,
        )
    }
}

#[cfg(not(feature = "mojo"))]
pub(super) fn translate_chat_request_to_anthropic(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    translate_chat_request_to_anthropic_rust(input)
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

#[cfg(feature = "mojo")]
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
    let id = json_fragment(&Value::String(
        value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("resp_anthropic")
            .to_string(),
    ))?;
    let model = json_fragment(&Value::String(
        value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
    ))?;
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
    input.id = Some(&id);
    input.model = Some(&model);
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

#[cfg(any(not(feature = "mojo"), test))]
fn anthropic_messages(value: Option<&Value>) -> Result<(Vec<String>, Vec<Value>), String> {
    let Some(messages) = value.and_then(Value::as_array) else {
        return Err("translated Responses request must contain messages".to_string());
    };
    let mut system = Vec::new();
    let mut translated = Vec::new();
    for message in messages {
        append_anthropic_message(message, &mut system, &mut translated)?;
    }
    if translated.is_empty() {
        return Err("Responses request must contain at least one user or assistant message".into());
    }
    Ok((system, translated))
}

#[cfg(any(not(feature = "mojo"), test))]
fn append_anthropic_message(
    message: &Value,
    system: &mut Vec<String>,
    translated: &mut Vec<Value>,
) -> Result<(), String> {
    let Some(object) = message.as_object() else {
        return Err("translated message must be an object".to_string());
    };
    let source_role = object.get("role").and_then(Value::as_str).unwrap_or("user");
    if source_role == "system" || source_role == "developer" {
        if let Some(text) = object.get("content").and_then(Value::as_str) {
            system.push(text.to_string());
        }
        return Ok(());
    }
    let role = if source_role == "assistant" {
        "assistant"
    } else {
        "user"
    };
    let blocks = anthropic_message_blocks(object)?;
    if !blocks.is_empty() {
        append_message(translated, role, blocks);
    }
    Ok(())
}

#[cfg(any(not(feature = "mojo"), test))]
fn anthropic_message_blocks(object: &Map<String, Value>) -> Result<Vec<Value>, String> {
    let mut blocks = Vec::new();
    if object.get("role").and_then(Value::as_str) != Some("tool")
        && let Some(text) = object.get("content").and_then(Value::as_str)
        && !text.is_empty()
    {
        blocks.push(json!({"type": "text", "text": text}));
    }
    if let Some(tool_calls) = object.get("tool_calls").and_then(Value::as_array) {
        blocks.extend(anthropic_tool_call_blocks(object, tool_calls)?);
    }
    if object.get("role").and_then(Value::as_str) == Some("tool") {
        blocks.push(json!({
            "type": "tool_result",
            "tool_use_id": object.get("tool_call_id").and_then(Value::as_str).unwrap_or("call_prodex"),
            "content": object.get("content").and_then(Value::as_str).unwrap_or(""),
        }));
    }
    Ok(blocks)
}

#[cfg(any(not(feature = "mojo"), test))]
fn anthropic_tool_call_blocks(
    object: &Map<String, Value>,
    tool_calls: &[Value],
) -> Result<Vec<Value>, String> {
    tool_calls
        .iter()
        .map(|tool_call| anthropic_tool_call_block(object, tool_call))
        .collect()
}

#[cfg(any(not(feature = "mojo"), test))]
fn anthropic_tool_call_block(
    object: &Map<String, Value>,
    tool_call: &Value,
) -> Result<Value, String> {
    let function = tool_call
        .get("function")
        .and_then(Value::as_object)
        .ok_or_else(|| "function call must contain function".to_string())?;
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "function call must contain name".to_string())?;
    let name = anthropic_tool_name(object.get("namespace").and_then(Value::as_str), name);
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    let input: Value = serde_json::from_str(arguments)
        .map_err(|_| "function call arguments must be valid JSON".to_string())?;
    if !input.is_object() {
        return Err("function call arguments must be a JSON object".to_string());
    }
    Ok(json!({
        "type": "tool_use",
        "id": tool_call.get("id").and_then(Value::as_str).unwrap_or("call_prodex"),
        "name": name,
        "input": input,
    }))
}

#[cfg(any(not(feature = "mojo"), test))]
fn append_message(messages: &mut Vec<Value>, role: &str, blocks: Vec<Value>) {
    if let Some(previous) = messages.last_mut()
        && previous.get("role").and_then(Value::as_str) == Some(role)
        && let Some(content) = previous.get_mut("content").and_then(Value::as_array_mut)
    {
        if blocks
            .first()
            .and_then(|block| block.get("type"))
            .and_then(Value::as_str)
            == Some("tool_result")
        {
            let insert_at = content
                .iter()
                .take_while(|block| {
                    block.get("type").and_then(Value::as_str) == Some("tool_result")
                })
                .count();
            content.splice(insert_at..insert_at, blocks);
        } else {
            content.extend(blocks);
        }
        return;
    }
    messages.push(json!({"role": role, "content": blocks}));
}

#[cfg(any(not(feature = "mojo"), test))]
fn anthropic_tools(value: &Value) -> Result<Vec<Value>, String> {
    let Some(tools) = value.as_array() else {
        return Err("Responses `tools` must be an array".to_string());
    };
    tools
        .iter()
        .map(|tool| {
            let object = tool
                .as_object()
                .ok_or_else(|| "Responses function tool must be an object".to_string())?;
            let function = object
                .get("function")
                .and_then(Value::as_object)
                .unwrap_or(object);
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| "Responses function tool must contain name".to_string())?;
            let name = anthropic_tool_name(function.get("namespace").and_then(Value::as_str), name);
            let mut translated = json!({
                "name": name,
                "input_schema": function
                    .get("parameters")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object", "properties": {}})),
            });
            if let Some(description) = function.get("description") {
                translated["description"] = description.clone();
            }
            Ok(translated)
        })
        .collect()
}

#[cfg(any(not(feature = "mojo"), test))]
fn anthropic_tool_choice(value: &Value) -> Result<Option<Value>, String> {
    match value {
        Value::String(choice) => match choice.as_str() {
            "auto" => Ok(Some(json!({"type": "auto"}))),
            "required" => Ok(Some(json!({"type": "any"}))),
            "none" => Ok(None),
            _ => Err(format!("unsupported Responses tool_choice `{choice}`")),
        },
        Value::Object(object) if object.get("type").and_then(Value::as_str) == Some("function") => {
            let name = object
                .get("name")
                .or_else(|| {
                    object
                        .get("function")
                        .and_then(|function| function.get("name"))
                })
                .and_then(Value::as_str)
                .ok_or_else(|| "function tool_choice must contain name".to_string())?;
            let name = anthropic_tool_name(object.get("namespace").and_then(Value::as_str), name);
            Ok(Some(json!({"type": "tool", "name": name})))
        }
        _ => Err("unsupported Responses tool_choice shape".to_string()),
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn anthropic_tool_name(namespace: Option<&str>, name: &str) -> String {
    namespace
        .filter(|namespace| !namespace.is_empty())
        .map(|namespace| format!("{namespace}--{name}"))
        .or_else(|| {
            name.rsplit_once('.')
                .filter(|(namespace, name)| !namespace.is_empty() && !name.is_empty())
                .map(|(namespace, name)| format!("{namespace}--{name}"))
        })
        .unwrap_or_else(|| name.to_string())
}

#[cfg(any(not(feature = "mojo"), test))]
fn responses_sse_event(name: &str, value: Value) -> String {
    format!("event: {name}\ndata: {value}\n\n")
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

#[cfg(all(test, feature = "mojo"))]
#[path = "messages/mojo_request_tests.rs"]
mod mojo_request_tests;

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
