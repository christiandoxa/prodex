pub(super) use super::deepseek_sse::RuntimeDeepSeekChatSseReader;
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use anyhow::{Context, Result};
use prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) type RuntimeDeepSeekConversationStore =
    Arc<Mutex<BTreeMap<String, Vec<serde_json::Value>>>>;
pub(super) type RuntimeDeepSeekPendingMessages = Arc<Mutex<BTreeMap<u64, Vec<serde_json::Value>>>>;

pub(super) struct RuntimeDeepSeekTranslatedRequest {
    pub(super) body: Vec<u8>,
    pub(super) messages: Vec<serde_json::Value>,
}

pub(super) fn runtime_deepseek_chat_request_body(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
) -> Result<RuntimeDeepSeekTranslatedRequest> {
    let value: serde_json::Value =
        serde_json::from_slice(body).context("failed to parse Codex Responses request JSON")?;
    let mut request = serde_json::Map::new();
    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(SUPER_DEEPSEEK_DEFAULT_MODEL);
    request.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    request.insert(
        "stream".to_string(),
        serde_json::Value::Bool(
            value
                .get("stream")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        ),
    );
    let messages = runtime_deepseek_messages_from_responses_request(&value, conversations)
        .unwrap_or_else(|| {
            vec![serde_json::json!({
                "role": "user",
                "content": "",
            })]
        });
    request.insert(
        "messages".to_string(),
        serde_json::Value::Array(messages.clone()),
    );
    let thinking_enabled = runtime_deepseek_thinking_enabled(&value);
    runtime_deepseek_apply_reasoning_from_responses_request(&value, &mut request);
    if let Some(tools) = runtime_deepseek_tools_from_responses_request(&value) {
        request.insert("tools".to_string(), serde_json::Value::Array(tools));
    }
    if let Some(tool_choice) =
        runtime_deepseek_tool_choice_from_responses_request(&value, thinking_enabled)
    {
        request.insert("tool_choice".to_string(), tool_choice);
    }
    for (from, to) in [
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("max_output_tokens", "max_tokens"),
    ] {
        if let Some(next) = value.get(from) {
            request.insert(to.to_string(), next.clone());
        }
    }
    let body = serde_json::to_vec(&serde_json::Value::Object(request))
        .context("failed to serialize DeepSeek chat request JSON")?;
    Ok(RuntimeDeepSeekTranslatedRequest { body, messages })
}

fn runtime_deepseek_messages_from_responses_request(
    value: &serde_json::Value,
    conversations: &RuntimeDeepSeekConversationStore,
) -> Option<Vec<serde_json::Value>> {
    let mut messages = Vec::new();
    let mut history_messages = Vec::new();
    let mut has_history = false;
    if let Some(previous_response_id) = value
        .get("previous_response_id")
        .and_then(serde_json::Value::as_str)
        && let Ok(conversations) = conversations.lock()
        && let Some(history) = conversations.get(previous_response_id)
    {
        history_messages.extend(history.iter().cloned());
        has_history = true;
    }
    if !has_history
        && let Some(call_id) = runtime_deepseek_first_function_call_output_call_id(value)
        && let Some(history) = runtime_deepseek_history_for_tool_call(conversations, &call_id)
    {
        history_messages.extend(history);
    }
    if let Some(instructions) = value
        .get("instructions")
        .and_then(serde_json::Value::as_str)
        && !instructions.trim().is_empty()
        && !runtime_deepseek_history_has_system_message(&history_messages, instructions)
    {
        messages.push(serde_json::json!({
            "role": "system",
            "content": instructions,
        }));
    }
    let replayed_tool_call_ids = runtime_deepseek_tool_call_ids(&history_messages);
    let replayed_tool_output_call_ids = runtime_deepseek_tool_output_call_ids(&history_messages);
    let replayed_message_signatures = runtime_deepseek_message_signatures(&history_messages);
    messages.extend(history_messages);
    match value.get("input") {
        Some(serde_json::Value::String(text)) => {
            messages.push(serde_json::json!({
                "role": "user",
                "content": text,
            }));
        }
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                runtime_deepseek_push_message_from_responses_item(
                    item,
                    &mut messages,
                    &replayed_tool_call_ids,
                    &replayed_tool_output_call_ids,
                    &replayed_message_signatures,
                );
            }
        }
        _ => {}
    }
    if messages.is_empty() {
        None
    } else {
        Some(messages)
    }
}

fn runtime_deepseek_history_has_system_message(
    history: &[serde_json::Value],
    content: &str,
) -> bool {
    history.iter().any(|message| {
        message.get("role").and_then(serde_json::Value::as_str) == Some("system")
            && message.get("content").and_then(serde_json::Value::as_str) == Some(content)
    })
}

fn runtime_deepseek_first_function_call_output_call_id(
    value: &serde_json::Value,
) -> Option<String> {
    value
        .get("input")?
        .as_array()?
        .iter()
        .filter_map(serde_json::Value::as_object)
        .find_map(|object| {
            (object.get("type").and_then(serde_json::Value::as_str) == Some("function_call_output"))
                .then(|| runtime_deepseek_json_string(object, &["call_id", "tool_call_id", "id"]))
                .flatten()
        })
        .filter(|call_id| !call_id.trim().is_empty())
}

fn runtime_deepseek_history_for_tool_call(
    conversations: &RuntimeDeepSeekConversationStore,
    call_id: &str,
) -> Option<Vec<serde_json::Value>> {
    let conversations = conversations.lock().ok()?;
    conversations
        .values()
        .rev()
        .find(|history| runtime_deepseek_history_has_tool_call(history, call_id))
        .cloned()
}

fn runtime_deepseek_history_has_tool_call(history: &[serde_json::Value], call_id: &str) -> bool {
    history.iter().any(|message| {
        message
            .get("tool_calls")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|tool_calls| {
                tool_calls.iter().any(|tool_call| {
                    tool_call.get("id").and_then(serde_json::Value::as_str) == Some(call_id)
                })
            })
    })
}

fn runtime_deepseek_tool_call_ids(history: &[serde_json::Value]) -> BTreeSet<String> {
    history
        .iter()
        .filter_map(|message| {
            message
                .get("tool_calls")
                .and_then(serde_json::Value::as_array)
        })
        .flat_map(|tool_calls| tool_calls.iter())
        .filter_map(|tool_call| tool_call.get("id").and_then(serde_json::Value::as_str))
        .filter(|call_id| !call_id.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn runtime_deepseek_tool_output_call_ids(history: &[serde_json::Value]) -> BTreeSet<String> {
    history
        .iter()
        .filter(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("tool"))
        .filter_map(|message| {
            message
                .get("tool_call_id")
                .and_then(serde_json::Value::as_str)
        })
        .filter(|call_id| !call_id.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn runtime_deepseek_message_signatures(
    history: &[serde_json::Value],
) -> BTreeSet<(String, String)> {
    history
        .iter()
        .filter_map(|message| {
            let role = runtime_deepseek_chat_role(
                message.get("role").and_then(serde_json::Value::as_str)?,
            );
            let content = message.get("content").and_then(serde_json::Value::as_str)?;
            (!content.trim().is_empty()).then(|| (role.to_string(), content.to_string()))
        })
        .collect()
}

fn runtime_deepseek_chat_role(role: &str) -> &str {
    match role {
        "assistant" | "system" | "tool" => role,
        "developer" => "system",
        _ => "user",
    }
}

fn runtime_deepseek_push_message_from_responses_item(
    item: &serde_json::Value,
    messages: &mut Vec<serde_json::Value>,
    replayed_tool_call_ids: &BTreeSet<String>,
    replayed_tool_output_call_ids: &BTreeSet<String>,
    replayed_message_signatures: &BTreeSet<(String, String)>,
) {
    let Some(object) = item.as_object() else {
        return;
    };
    match object.get("type").and_then(serde_json::Value::as_str) {
        Some("message") => {
            let role = object
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("user");
            let role = runtime_deepseek_chat_role(role);
            let text = runtime_deepseek_responses_content_text(object.get("content"));
            if replayed_message_signatures.contains(&(role.to_string(), text.clone())) {
                return;
            }
            if !text.trim().is_empty() {
                messages.push(serde_json::json!({
                    "role": role,
                    "content": text,
                }));
            }
        }
        Some("function_call") => {
            let call_id = runtime_deepseek_json_string(object, &["call_id", "id"])
                .unwrap_or_else(|| "call_0".to_string());
            if replayed_tool_call_ids.contains(&call_id) {
                return;
            }
            let name = runtime_deepseek_json_string(object, &["name"]).unwrap_or_else(|| {
                runtime_deepseek_json_string_at_path(object, &["function", "name"])
                    .unwrap_or_else(|| "tool_call".to_string())
            });
            let arguments =
                runtime_deepseek_json_string(object, &["arguments"]).unwrap_or_else(|| {
                    object
                        .get("arguments")
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "{}".to_string())
                });
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": call_id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": arguments,
                    },
                }],
            }));
        }
        Some("function_call_output") => {
            let call_id = runtime_deepseek_json_string(object, &["call_id", "tool_call_id", "id"])
                .unwrap_or_else(|| "call_0".to_string());
            if replayed_tool_output_call_ids.contains(&call_id) {
                return;
            }
            let output = runtime_deepseek_json_string(object, &["output"])
                .or_else(|| object.get("output").map(|value| value.to_string()))
                .unwrap_or_default();
            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": output,
            }));
        }
        Some(other) => {
            let text = runtime_deepseek_responses_content_text(object.get("content"))
                .trim()
                .to_string();
            if !text.is_empty() {
                messages.push(serde_json::json!({
                    "role": "user",
                    "content": format!("{other}: {text}"),
                }));
            }
        }
        None => {}
    }
}

fn runtime_deepseek_responses_content_text(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Array(parts)) => parts
            .iter()
            .filter_map(runtime_deepseek_responses_content_part_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn runtime_deepseek_responses_content_part_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Object(object) => object
            .get("text")
            .and_then(serde_json::Value::as_str)
            .or_else(|| object.get("input_text").and_then(serde_json::Value::as_str))
            .or_else(|| {
                object
                    .get("output_text")
                    .and_then(serde_json::Value::as_str)
            })
            .map(str::to_string),
        _ => None,
    }
}

fn runtime_deepseek_tools_from_responses_request(
    value: &serde_json::Value,
) -> Option<Vec<serde_json::Value>> {
    let tools = value.get("tools")?.as_array()?;
    let tools = tools
        .iter()
        .filter_map(runtime_deepseek_tool_from_responses_tool)
        .collect::<Vec<_>>();
    if tools.is_empty() { None } else { Some(tools) }
}

fn runtime_deepseek_tool_from_responses_tool(
    tool: &serde_json::Value,
) -> Option<serde_json::Value> {
    let object = tool.as_object()?;
    if object.get("type").and_then(serde_json::Value::as_str) != Some("function") {
        return None;
    }
    let function_object = object
        .get("function")
        .and_then(serde_json::Value::as_object)
        .unwrap_or(object);
    let name = runtime_deepseek_json_string(function_object, &["name"])
        .filter(|name| !name.trim().is_empty())?;
    let mut function = serde_json::Map::new();
    function.insert("name".to_string(), serde_json::Value::String(name));
    if let Some(description) = runtime_deepseek_json_string(function_object, &["description"])
        .filter(|description| !description.trim().is_empty())
    {
        function.insert(
            "description".to_string(),
            serde_json::Value::String(description),
        );
    }
    if let Some(parameters) = function_object.get("parameters") {
        function.insert("parameters".to_string(), parameters.clone());
    }

    Some(serde_json::json!({
        "type": "function",
        "function": function,
    }))
}

fn runtime_deepseek_tool_choice_from_responses_request(
    value: &serde_json::Value,
    thinking_enabled: bool,
) -> Option<serde_json::Value> {
    if thinking_enabled {
        return None;
    }
    let choice = value.get("tool_choice")?;
    if let Some(choice) = choice.as_str() {
        return matches!(choice, "auto" | "none" | "required")
            .then(|| serde_json::Value::String(choice.to_string()));
    }

    let object = choice.as_object()?;
    let choice_type = object.get("type").and_then(serde_json::Value::as_str)?;
    if choice_type != "function" {
        return None;
    }
    let name = runtime_deepseek_json_string(object, &["name"])
        .or_else(|| runtime_deepseek_json_string_at_path(object, &["function", "name"]))
        .filter(|name| !name.trim().is_empty())?;
    Some(serde_json::json!({
        "type": "function",
        "function": {
            "name": name,
        },
    }))
}

fn runtime_deepseek_apply_reasoning_from_responses_request(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
) {
    let effort = value
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("reasoning_effort")
                .and_then(serde_json::Value::as_str)
        });
    match effort.and_then(runtime_deepseek_reasoning_effort) {
        Some(Some(effort)) => {
            request.insert(
                "thinking".to_string(),
                serde_json::json!({"type": "enabled"}),
            );
            request.insert(
                "reasoning_effort".to_string(),
                serde_json::Value::String(effort.to_string()),
            );
        }
        Some(None) => {
            request.insert(
                "thinking".to_string(),
                serde_json::json!({"type": "disabled"}),
            );
        }
        None => {}
    }
}

fn runtime_deepseek_thinking_enabled(value: &serde_json::Value) -> bool {
    runtime_deepseek_reasoning_effort_from_responses_request(value)
        .and_then(runtime_deepseek_reasoning_effort)
        .is_some_and(|effort| effort.is_some())
}

fn runtime_deepseek_reasoning_effort_from_responses_request(
    value: &serde_json::Value,
) -> Option<&str> {
    value
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("reasoning_effort")
                .and_then(serde_json::Value::as_str)
        })
}

fn runtime_deepseek_reasoning_effort(effort: &str) -> Option<Option<&'static str>> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "xhigh" | "max" => Some(Some("max")),
        "high" | "medium" | "low" => Some(Some("high")),
        "minimal" | "none" => Some(None),
        _ => None,
    }
}

fn runtime_deepseek_json_string(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

fn runtime_deepseek_json_string_at_path(
    object: &serde_json::Map<String, serde_json::Value>,
    path: &[&str],
) -> Option<String> {
    let mut value = object.get(*path.first()?)?;
    for key in path.iter().skip(1) {
        value = value.get(*key)?;
    }
    value.as_str().map(str::to_string)
}

pub(super) fn runtime_deepseek_chat_buffered_response_parts(
    status: u16,
    mut response: reqwest::blocking::Response,
    request_id: u64,
    conversation_messages: Vec<serde_json::Value>,
    conversations: &RuntimeDeepSeekConversationStore,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let mut body = Vec::new();
    response
        .read_to_end(&mut body)
        .context("failed to read DeepSeek chat response body")?;
    let value: serde_json::Value =
        serde_json::from_slice(&body).context("failed to parse DeepSeek chat response JSON")?;
    let response = runtime_deepseek_responses_value_from_chat_value(&value, request_id);
    if let Some(response_id) = response.get("id").and_then(serde_json::Value::as_str) {
        runtime_deepseek_store_conversation(
            conversations,
            response_id,
            conversation_messages,
            runtime_deepseek_chat_assistant_messages_from_response_value(&value),
        );
    }
    let body = serde_json::to_vec(&response).context("failed to serialize Responses JSON")?;
    Ok(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    })
}

pub(super) fn runtime_deepseek_take_pending_messages(
    pending: &RuntimeDeepSeekPendingMessages,
    request_id: u64,
) -> Vec<serde_json::Value> {
    pending
        .lock()
        .ok()
        .and_then(|mut pending| pending.remove(&request_id))
        .unwrap_or_default()
}

pub(super) fn runtime_deepseek_store_conversation(
    conversations: &RuntimeDeepSeekConversationStore,
    response_id: &str,
    mut messages: Vec<serde_json::Value>,
    assistant_messages: Vec<serde_json::Value>,
) {
    if response_id.trim().is_empty() {
        return;
    }
    messages.extend(
        assistant_messages
            .into_iter()
            .map(runtime_deepseek_normalize_assistant_tool_call_content),
    );
    if let Ok(mut conversations) = conversations.lock() {
        conversations.insert(response_id.to_string(), messages);
    }
}

fn runtime_deepseek_normalize_assistant_tool_call_content(
    mut message: serde_json::Value,
) -> serde_json::Value {
    let is_assistant = message.get("role").and_then(serde_json::Value::as_str) == Some("assistant");
    let has_tool_calls = message
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tool_calls| !tool_calls.is_empty());
    if is_assistant
        && has_tool_calls
        && message
            .get("content")
            .is_none_or(serde_json::Value::is_null)
    {
        message["content"] = serde_json::Value::String(String::new());
    }
    message
}

fn runtime_deepseek_chat_assistant_messages_from_response_value(
    value: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let Some(message) = value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
    else {
        return Vec::new();
    };
    let content = message
        .get("content")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let reasoning_content = message
        .get("reasoning_content")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let tool_calls = message.get("tool_calls").cloned();
    if content.is_empty() && reasoning_content.is_empty() && tool_calls.is_none() {
        return Vec::new();
    }
    let has_tool_calls = tool_calls.is_some();
    let mut assistant = serde_json::json!({
        "role": "assistant",
        "content": if content.is_empty() {
            if has_tool_calls {
                serde_json::Value::String(String::new())
            } else {
                serde_json::Value::Null
            }
        } else {
            serde_json::Value::String(content.to_string())
        },
    });
    if !reasoning_content.is_empty() {
        assistant["reasoning_content"] = serde_json::Value::String(reasoning_content.to_string());
    }
    if let Some(tool_calls) = tool_calls {
        assistant["tool_calls"] = tool_calls;
    }
    vec![assistant]
}

fn runtime_deepseek_responses_value_from_chat_value(
    value: &serde_json::Value,
    request_id: u64,
) -> serde_json::Value {
    let response_id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("resp_deepseek_{request_id}"));
    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(SUPER_DEEPSEEK_DEFAULT_MODEL);
    let message = value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"));
    let mut output = Vec::new();
    if let Some(content) = message
        .and_then(|message| message.get("content"))
        .and_then(serde_json::Value::as_str)
        .filter(|content| !content.is_empty())
    {
        output.push(serde_json::json!({
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": content,
            }],
        }));
    }
    if let Some(tool_calls) = message
        .and_then(|message| message.get("tool_calls"))
        .and_then(serde_json::Value::as_array)
    {
        for tool_call in tool_calls {
            if let Some(item) = runtime_deepseek_responses_tool_call_item(tool_call) {
                output.push(item);
            }
        }
    }
    let mut response = serde_json::json!({
        "id": response_id,
        "object": "response",
        "created_at": runtime_deepseek_created_at(),
        "model": model,
        "output": output,
    });
    if let Some(usage) = value
        .get("usage")
        .and_then(runtime_deepseek_responses_usage)
    {
        response["usage"] = usage;
    }
    response
}

fn runtime_deepseek_responses_tool_call_item(
    tool_call: &serde_json::Value,
) -> Option<serde_json::Value> {
    let call_id = tool_call
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("call_0");
    let function = tool_call.get("function")?;
    let name = function
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool_call");
    let arguments = function
        .get("arguments")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("{}");
    Some(serde_json::json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": arguments,
    }))
}

pub(super) fn runtime_deepseek_responses_usage(
    usage: &serde_json::Value,
) -> Option<serde_json::Value> {
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
    Some(serde_json::json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": total_tokens,
    }))
}

pub(super) fn runtime_deepseek_created_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "deepseek_rewrite_tests.rs"]
mod deepseek_rewrite_tests;
