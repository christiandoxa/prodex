use super::super::openai_chat_compat::translate_responses_request_to_chat;
use crate::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformLoss,
    ProviderTransformResult, ProviderWireFormat,
};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

#[path = "messages/web_search.rs"]
mod web_search;

use web_search::{
    anthropic_tool_usage, anthropic_web_search_call, anthropic_web_search_tool,
    merge_anthropic_web_search_result,
};

const DEFAULT_MAX_TOKENS: u64 = 4096;

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
                _ => return rejected_chat("Responses `stop` must be a string or array"),
            },
        );
    }
    let mut degradation_details = BTreeMap::new();
    let mut tools = match chat.get("tools") {
        Some(tools) => match anthropic_tools(tools) {
            Ok(tools) => tools,
            Err(reason) => return rejected_chat(reason),
        },
        None => Vec::new(),
    };
    if let Some(options) = chat.get("web_search_options") {
        match anthropic_web_search_tool(options) {
            Ok((tool, ignored_context_size)) => {
                tools.push(tool);
                if let Some(context_size) = ignored_context_size {
                    degradation_details.insert(
                        "web_search_options.search_context_size".to_string(),
                        json!({"from": context_size, "to": "provider_default"}),
                    );
                }
            }
            Err(reason) => return rejected_chat(reason),
        }
    }
    if !tools.is_empty() {
        request.insert("tools".to_string(), Value::Array(tools));
    }
    if let Some(tool_choice) = chat.get("tool_choice") {
        match anthropic_tool_choice(tool_choice) {
            Ok(Some(choice)) => {
                request.insert("tool_choice".to_string(), choice);
            }
            Ok(None) => {
                request.remove("tools");
                degradation_details.clear();
            }
            Err(reason) => return rejected_chat(reason),
        }
    }

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
    let mut output = Vec::new();
    let mut text = Vec::new();
    for block in content {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                let Some(value) = block.get("text").and_then(Value::as_str) else {
                    return rejected_response("Anthropic text block must contain text");
                };
                text.push(json!({"type": "output_text", "text": value}));
            }
            Some("tool_use") => {
                flush_text_output(&mut output, &mut text);
                let Some(id) = block.get("id").and_then(Value::as_str) else {
                    return rejected_response("Anthropic tool_use block must contain id");
                };
                let Some(name) = block.get("name").and_then(Value::as_str) else {
                    return rejected_response("Anthropic tool_use block must contain name");
                };
                let arguments =
                    serde_json::to_string(block.get("input").unwrap_or(&Value::Object(Map::new())))
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
                output.push(item);
            }
            Some("server_tool_use") => {
                flush_text_output(&mut output, &mut text);
                match anthropic_web_search_call(block) {
                    Ok(item) => output.push(item),
                    Err(reason) => return rejected_response(reason),
                }
            }
            Some("web_search_tool_result") => {
                flush_text_output(&mut output, &mut text);
                merge_anthropic_web_search_result(&mut output, block);
            }
            Some("thinking") => {
                flush_text_output(&mut output, &mut text);
                if let Some(thinking) = block.get("thinking").and_then(Value::as_str) {
                    output.push(json!({
                        "type": "reasoning",
                        "summary": [{"type": "summary_text", "text": thinking}],
                    }));
                }
            }
            Some(kind) => {
                return rejected_response(format!(
                    "unsupported Anthropic Messages content block `{kind}`"
                ));
            }
            None => return rejected_response("Anthropic Messages content block requires type"),
        }
    }
    flush_text_output(&mut output, &mut text);

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

pub(super) fn translate_anthropic_stream_event_to_responses(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if input.endpoint != ProviderEndpoint::Responses {
        return unsupported(
            input.endpoint,
            "native Messages translation only supports responses",
        );
    }
    let event = String::from_utf8_lossy(&input.body);
    let Some(data) = event.lines().find_map(|line| line.strip_prefix("data: ")) else {
        return unsupported(
            ProviderEndpoint::Responses,
            "Anthropic SSE event must contain data: <json> framing",
        );
    };
    let value: Value = match serde_json::from_str(data) {
        Ok(value) => value,
        Err(error) => {
            return rejected_stream(format!("failed to parse Anthropic SSE JSON: {error}"));
        }
    };
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let translated = match event_type {
        "message_start" => {
            let message = value.get("message").cloned().unwrap_or(Value::Null);
            responses_sse_event(
                "response.created",
                json!({
                    "type": "response.created",
                    "response": {
                        "id": message.get("id").and_then(Value::as_str).unwrap_or("resp_anthropic"),
                        "object": "response",
                        "created_at": unix_now_secs(),
                        "model": message.get("model").and_then(Value::as_str).unwrap_or("unknown"),
                        "output": [],
                    }
                }),
            )
        }
        "content_block_start" => {
            let Some(block) = value.get("content_block") else {
                return rejected_stream("Anthropic content_block_start requires content_block");
            };
            let index = value.get("index").cloned().unwrap_or(Value::from(0));
            match block.get("type").and_then(Value::as_str) {
                Some("text") => responses_sse_event(
                    "response.output_item.added",
                    json!({
                        "type": "response.output_item.added",
                        "output_index": index,
                        "item": {"type": "message", "role": "assistant", "content": []},
                    }),
                ),
                Some("tool_use") => responses_sse_event(
                    "response.output_item.added",
                    json!({
                        "type": "response.output_item.added",
                        "output_index": index,
                        "item": {
                            "type": "function_call",
                            "call_id": block.get("id").cloned().unwrap_or(Value::Null),
                            "name": block.get("name").cloned().unwrap_or(Value::Null),
                            "arguments": "",
                        },
                    }),
                ),
                Some("server_tool_use")
                    if block.get("name").and_then(Value::as_str) == Some("web_search") =>
                {
                    let mut item = match anthropic_web_search_call(block) {
                        Ok(item) => item,
                        Err(reason) => return rejected_stream(reason),
                    };
                    item["status"] = Value::String("in_progress".to_string());
                    responses_sse_event(
                        "response.output_item.added",
                        json!({
                            "type": "response.output_item.added",
                            "output_index": index,
                            "item": item,
                        }),
                    )
                }
                Some("web_search_tool_result") => return empty_lossless_stream(),
                Some("thinking") => responses_sse_event(
                    "response.output_item.added",
                    json!({
                        "type": "response.output_item.added",
                        "output_index": index,
                        "item": {"type": "reasoning", "summary": []},
                    }),
                ),
                Some(_) => return empty_lossless_stream(),
                None => return rejected_stream("Anthropic content block requires type"),
            }
        }
        "content_block_delta" => match value
            .get("delta")
            .and_then(|delta| delta.get("type"))
            .and_then(Value::as_str)
        {
            Some("text_delta") => responses_sse_event(
                "response.output_text.delta",
                json!({
                    "type": "response.output_text.delta",
                    "output_index": value.get("index").cloned().unwrap_or(Value::from(0)),
                    "delta": value.pointer("/delta/text").and_then(Value::as_str).unwrap_or(""),
                }),
            ),
            Some("input_json_delta") => responses_sse_event(
                "response.function_call_arguments.delta",
                json!({
                    "type": "response.function_call_arguments.delta",
                    "output_index": value.get("index").cloned().unwrap_or(Value::from(0)),
                    "delta": value.pointer("/delta/partial_json").and_then(Value::as_str).unwrap_or(""),
                }),
            ),
            Some("thinking_delta") => responses_sse_event(
                "response.reasoning_summary_text.delta",
                json!({
                    "type": "response.reasoning_summary_text.delta",
                    "output_index": value.get("index").cloned().unwrap_or(Value::from(0)),
                    "delta": value.pointer("/delta/thinking").and_then(Value::as_str).unwrap_or(""),
                }),
            ),
            Some(_) => return empty_lossless_stream(),
            None => return rejected_stream("Anthropic content_block_delta requires delta.type"),
        },
        "message_stop" => responses_sse_event(
            "response.completed",
            json!({
                "type": "response.completed"
            }),
        ),
        "error" => responses_sse_event(
            "error",
            json!({
                "type": "error",
                "error": value.get("error").cloned().unwrap_or(Value::Null),
            }),
        ),
        "ping" | "content_block_stop" | "message_delta" => return empty_lossless_stream(),
        _ => return empty_lossless_stream(),
    };
    ProviderTransformResult::lossless(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        ProviderWireFormat::AnthropicMessages,
        ProviderWireFormat::OpenAiResponses,
        translated.into_bytes(),
    )
}

fn anthropic_messages(value: Option<&Value>) -> Result<(Vec<String>, Vec<Value>), String> {
    let Some(messages) = value.and_then(Value::as_array) else {
        return Err("translated Responses request must contain messages".to_string());
    };
    let mut system = Vec::new();
    let mut translated = Vec::new();
    for message in messages {
        let Some(object) = message.as_object() else {
            return Err("translated message must be an object".to_string());
        };
        let role = object.get("role").and_then(Value::as_str).unwrap_or("user");
        if role == "system" || role == "developer" {
            if let Some(text) = object.get("content").and_then(Value::as_str) {
                system.push(text.to_string());
            }
            continue;
        }
        let role = if role == "assistant" {
            "assistant"
        } else {
            "user"
        };
        let mut blocks = Vec::new();
        if object.get("role").and_then(Value::as_str) != Some("tool")
            && let Some(text) = object.get("content").and_then(Value::as_str)
            && !text.is_empty()
        {
            blocks.push(json!({"type": "text", "text": text}));
        }
        if let Some(tool_calls) = object.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                let function = tool_call
                    .get("function")
                    .and_then(Value::as_object)
                    .ok_or_else(|| "function call must contain function".to_string())?;
                let name = function
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "function call must contain name".to_string())?;
                let name =
                    anthropic_tool_name(object.get("namespace").and_then(Value::as_str), name);
                let arguments = function
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}");
                let input: Value = serde_json::from_str(arguments)
                    .map_err(|_| "function call arguments must be valid JSON".to_string())?;
                if !input.is_object() {
                    return Err("function call arguments must be a JSON object".to_string());
                }
                blocks.push(json!({
                    "type": "tool_use",
                    "id": tool_call.get("id").and_then(Value::as_str).unwrap_or("call_prodex"),
                    "name": name,
                    "input": input,
                }));
            }
        }
        if object.get("role").and_then(Value::as_str) == Some("tool") {
            blocks.push(json!({
                "type": "tool_result",
                "tool_use_id": object.get("tool_call_id").and_then(Value::as_str).unwrap_or("call_prodex"),
                "content": object.get("content").and_then(Value::as_str).unwrap_or(""),
            }));
        }
        if blocks.is_empty() {
            continue;
        }
        append_message(&mut translated, role, blocks);
    }
    if translated.is_empty() {
        return Err("Responses request must contain at least one user or assistant message".into());
    }
    Ok((system, translated))
}

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

fn flush_text_output(output: &mut Vec<Value>, text: &mut Vec<Value>) {
    if !text.is_empty() {
        output.push(json!({
            "type": "message",
            "role": "assistant",
            "content": std::mem::take(text),
        }));
    }
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
