pub(super) use super::deepseek_sse::RuntimeDeepSeekChatSseReader;
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use anyhow::{Context, Result};
use prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL;
use std::collections::BTreeMap;
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
    if let Some(tools) = runtime_deepseek_tools_from_responses_request(&value) {
        request.insert("tools".to_string(), serde_json::Value::Array(tools));
    }
    for (from, to) in [
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("max_output_tokens", "max_tokens"),
        ("tool_choice", "tool_choice"),
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
    if let Some(previous_response_id) = value
        .get("previous_response_id")
        .and_then(serde_json::Value::as_str)
        && let Ok(conversations) = conversations.lock()
        && let Some(history) = conversations.get(previous_response_id)
    {
        messages.extend(history.iter().cloned());
    }
    if let Some(instructions) = value
        .get("instructions")
        .and_then(serde_json::Value::as_str)
        && !instructions.trim().is_empty()
    {
        messages.push(serde_json::json!({
            "role": "system",
            "content": instructions,
        }));
    }
    match value.get("input") {
        Some(serde_json::Value::String(text)) => {
            messages.push(serde_json::json!({
                "role": "user",
                "content": text,
            }));
        }
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                runtime_deepseek_push_message_from_responses_item(item, &mut messages);
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

fn runtime_deepseek_push_message_from_responses_item(
    item: &serde_json::Value,
    messages: &mut Vec<serde_json::Value>,
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
            let role = match role {
                "assistant" | "system" | "tool" => role,
                "developer" => "system",
                _ => "user",
            };
            let text = runtime_deepseek_responses_content_text(object.get("content"));
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
                "content": null,
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
        .filter_map(|tool| {
            let object = tool.as_object()?;
            if object.get("type").and_then(serde_json::Value::as_str) != Some("function") {
                return None;
            }
            Some(serde_json::Value::Object(object.clone()))
        })
        .collect::<Vec<_>>();
    if tools.is_empty() { None } else { Some(tools) }
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
    messages.extend(assistant_messages);
    if let Ok(mut conversations) = conversations.lock() {
        conversations.insert(response_id.to_string(), messages);
    }
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
    let tool_calls = message.get("tool_calls").cloned();
    if content.is_empty() && tool_calls.is_none() {
        return Vec::new();
    }
    let mut assistant = serde_json::json!({
        "role": "assistant",
        "content": if content.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(content.to_string())
        },
    });
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
