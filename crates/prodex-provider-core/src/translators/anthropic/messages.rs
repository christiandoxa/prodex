use super::super::openai_chat_compat::translate_responses_request_to_chat;
#[cfg(feature = "mojo")]
use super::anthropic_mojo_value;
use crate::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformLoss,
    ProviderTransformResult, ProviderWireFormat,
};
#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "mojo")]
#[path = "messages/request_builder.rs"]
mod request_builder;
#[path = "messages/response.rs"]
mod response;
#[path = "messages/stream.rs"]
mod stream;
#[cfg(feature = "mojo")]
#[path = "messages/tool_shapes.rs"]
mod tool_shapes;
#[path = "messages/web_search.rs"]
mod web_search;

pub(super) use stream::translate_anthropic_stream_event_to_responses;
use web_search::{
    anthropic_tool_usage, anthropic_web_search_call, anthropic_web_search_tool,
    merge_anthropic_web_search_result,
};

const DEFAULT_MAX_TOKENS: u64 = 4096;

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
    let (request, degradation_details) = match build_anthropic_chat_request(&system, messages, chat)
    {
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
fn build_anthropic_chat_request(
    system: &[String],
    messages: Vec<Value>,
    chat: &Map<String, Value>,
) -> Result<AnthropicChatRequest, String> {
    let mut request = Map::new();
    request.insert(
        "model".to_string(),
        chat.get("model")
            .cloned()
            .unwrap_or_else(|| Value::String("auto".to_string())),
    );
    request.insert("messages".to_string(), Value::Array(messages));
    request.insert(
        "max_tokens".to_string(),
        chat.get("max_tokens")
            .cloned()
            .unwrap_or_else(|| Value::from(DEFAULT_MAX_TOKENS)),
    );
    request.insert(
        "stream".to_string(),
        Value::Bool(chat.get("stream").and_then(Value::as_bool).unwrap_or(false)),
    );
    if !system.is_empty() {
        request.insert("system".to_string(), Value::String(system.join("\n\n")));
    }
    for field in ["temperature", "top_p"] {
        if let Some(value) = chat.get(field) {
            request.insert(field.to_string(), value.clone());
        }
    }
    if let Some(stop) = chat.get("stop") {
        request.insert(
            "stop_sequences".to_string(),
            match stop {
                Value::String(_) => Value::Array(vec![stop.clone()]),
                Value::Array(_) => stop.clone(),
                _ => return Err("Responses `stop` must be a string or array".to_string()),
            },
        );
    }
    let mut degradation_details = BTreeMap::new();
    let mut tools = match chat.get("tools") {
        Some(tools) => anthropic_tools(tools)?,
        None => Vec::new(),
    };
    if let Some(options) = chat.get("web_search_options") {
        let (tool, ignored_context_size) = anthropic_web_search_tool(options)?;
        tools.push(tool);
        if let Some(context_size) = ignored_context_size {
            degradation_details.insert(
                "web_search_options.search_context_size".to_string(),
                json!({"from": context_size, "to": "provider_default"}),
            );
        }
    }
    if !tools.is_empty() {
        request.insert("tools".to_string(), Value::Array(tools));
    }
    if let Some(tool_choice) = chat.get("tool_choice") {
        match anthropic_tool_choice(tool_choice)? {
            Some(choice) => {
                request.insert("tool_choice".to_string(), choice);
            }
            None => {
                request.remove("tools");
                degradation_details.clear();
            }
        }
    }
    Ok((request, degradation_details))
}

#[cfg(feature = "mojo")]
fn json_fragment(value: &Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("Anthropic JSON fragment failed: {error}"))
}

#[cfg(feature = "mojo")]
use request_builder::build_anthropic_chat_request;

fn validate_anthropic_chat_fields(chat: &Map<String, Value>) -> Result<(), String> {
    for field in chat.keys() {
        match field.as_str() {
            "model" | "messages" | "max_tokens" | "stream" | "temperature" | "top_p" | "stop"
            | "tools" | "tool_choice" | "stream_options" | "web_search_options" => {}
            "parallel_tool_calls" if chat.get(field).and_then(Value::as_bool) == Some(true) => {}
            "parallel_tool_calls" => {
                return Err(
                    "Anthropic Messages only accepts `parallel_tool_calls=true`".to_string()
                );
            }
            _ => {
                return Err(format!(
                    "Anthropic Messages does not translate chat field `{field}`"
                ));
            }
        }
    }
    Ok(())
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

    let mut response = json!({
        "id": value.get("id").and_then(Value::as_str).unwrap_or("resp_anthropic"),
        "object": "response",
        "created_at": unix_now_secs(),
        "model": value.get("model").and_then(Value::as_str).unwrap_or("unknown"),
        "output": output,
    });
    if let Some(usage) = anthropic_usage(value.get("usage")) {
        response["usage"] = usage;
    }
    if let Some(tool_usage) = anthropic_tool_usage(value.get("usage")) {
        response["tool_usage"] = tool_usage;
    }
    if let Some(stop_reason) = value.get("stop_reason") {
        response["metadata"] = json!({"anthropic": {"stop_reason": stop_reason}});
    }

    ProviderTransformResult::lossless(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        ProviderWireFormat::AnthropicMessages,
        ProviderWireFormat::OpenAiResponses,
        serde_json::to_vec(&response).expect("Responses response serializes"),
    )
}

#[cfg(not(feature = "mojo"))]
fn anthropic_tool_use_item(block: &Value) -> Result<Value, String> {
    let Some(id) = block.get("id").and_then(Value::as_str) else {
        return Err("Anthropic tool_use block must contain id".to_string());
    };
    let Some(name) = block.get("name").and_then(Value::as_str) else {
        return Err("Anthropic tool_use block must contain name".to_string());
    };
    let arguments = serde_json::to_string(block.get("input").unwrap_or(&Value::Object(Map::new())))
        .expect("Anthropic tool input serializes");
    let (namespace, name) = crate::provider_core_split_flat_namespace_tool_name(name);
    let mut item = json!({
        "type": "function_call",
        "call_id": id,
        "name": name,
        "arguments": crate::provider_core_chat_compatible_rtk_wrapped_tool_arguments(
            block.get("name").and_then(Value::as_str).unwrap_or(&name),
            &arguments,
        ),
    });
    if let Some(namespace) = namespace {
        item["namespace"] = Value::String(namespace);
    }
    Ok(item)
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
        #[cfg(feature = "mojo")]
        append_message(translated, role, blocks)?;
        #[cfg(not(feature = "mojo"))]
        append_message(translated, role, blocks);
    }
    Ok(())
}

#[cfg(not(feature = "mojo"))]
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

#[cfg(feature = "mojo")]
fn anthropic_message_blocks(object: &Map<String, Value>) -> Result<Vec<Value>, String> {
    let mut blocks = Vec::new();
    if object.get("role").and_then(Value::as_str) != Some("tool")
        && let Some(text) = object.get("content").and_then(Value::as_str)
        && !text.is_empty()
    {
        let content = json_fragment(&Value::String(text.to_string()))?;
        let mut input =
            AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::TextBlock);
        input.content = Some(&content);
        blocks.push(anthropic_mojo_value(input)?);
    }
    if let Some(tool_calls) = object.get("tool_calls").and_then(Value::as_array) {
        blocks.extend(anthropic_tool_call_blocks(object, tool_calls)?);
    }
    if object.get("role").and_then(Value::as_str) == Some("tool") {
        let tool_use_id = object
            .get("tool_call_id")
            .and_then(Value::as_str)
            .unwrap_or("call_prodex");
        let tool_use_id = json_fragment(&Value::String(tool_use_id.to_string()))?;
        let content = json_fragment(&Value::String(
            object
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ))?;
        let mut input =
            AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ToolResultBlock);
        input.tool_use_id = Some(&tool_use_id);
        input.content = Some(&content);
        blocks.push(anthropic_mojo_value(input)?);
    }
    Ok(blocks)
}

#[cfg(not(feature = "mojo"))]
fn anthropic_tool_call_blocks(
    object: &Map<String, Value>,
    tool_calls: &[Value],
) -> Result<Vec<Value>, String> {
    tool_calls
        .iter()
        .map(|tool_call| anthropic_tool_call_block(object, tool_call))
        .collect()
}

#[cfg(feature = "mojo")]
fn anthropic_tool_call_blocks(
    object: &Map<String, Value>,
    tool_calls: &[Value],
) -> Result<Vec<Value>, String> {
    tool_calls
        .iter()
        .map(|tool_call| anthropic_tool_call_block(object, tool_call))
        .collect()
}

#[cfg(not(feature = "mojo"))]
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

#[cfg(feature = "mojo")]
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
    let id = json_fragment(&Value::String(
        tool_call
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("call_prodex")
            .to_string(),
    ))?;
    let name = json_fragment(&Value::String(name))?;
    let input_value = json_fragment(&input)?;
    let mut kernel =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ToolUseBlock);
    kernel.id = Some(&id);
    kernel.name = Some(&name);
    kernel.input = Some(&input_value);
    anthropic_mojo_value(kernel)
}

#[cfg(not(feature = "mojo"))]
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

#[cfg(feature = "mojo")]
fn append_message(messages: &mut Vec<Value>, role: &str, blocks: Vec<Value>) -> Result<(), String> {
    let existing = json_fragment(&Value::Array(std::mem::take(messages)))?;
    let role = json_fragment(&Value::String(role.to_string()))?;
    let blocks = json_fragment(&Value::Array(blocks))?;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::AppendMessage);
    input.messages = Some(&existing);
    input.role = Some(&role);
    input.blocks = Some(&blocks);
    let output = anthropic_mojo_value(input)?;
    *messages = output
        .as_array()
        .cloned()
        .ok_or_else(|| "Anthropic append-message kernel returned a non-array".to_string())?;
    Ok(())
}

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

fn anthropic_usage(value: Option<&Value>) -> Option<Value> {
    let value = value?.as_object()?;
    let input = value
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = value
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Some(json!({
        "input_tokens": input,
        "output_tokens": output,
        "total_tokens": input.saturating_add(output),
    }))
}

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

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
