use self::json_helpers::{runtime_deepseek_json_string, runtime_deepseek_json_string_at_path};
#[cfg(test)]
use self::response::{
    runtime_deepseek_chat_assistant_messages_from_response_value,
    runtime_deepseek_responses_value_from_chat_value,
};
pub(super) use self::response::{
    runtime_deepseek_chat_buffered_response_parts, runtime_deepseek_created_at,
    runtime_deepseek_normalize_assistant_tool_call_content, runtime_deepseek_responses_usage,
    runtime_deepseek_store_conversation, runtime_deepseek_take_pending_messages,
};
pub(super) use self::rtk::runtime_deepseek_rtk_wrapped_tool_arguments;
use self::thinking::runtime_deepseek_normalize_thinking_tool_call_messages;
use self::tool_adjacency::runtime_deepseek_repair_tool_call_adjacency;
use self::tools::{
    runtime_deepseek_tool_choice_from_responses_request,
    runtime_deepseek_tools_from_responses_request,
};
pub(super) use super::deepseek_sse::RuntimeDeepSeekChatSseReader;
use super::provider_bridge::{RuntimeProviderBridgeKind, runtime_provider_canonical_model};
use anyhow::{Context, Result};
use prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

mod json_helpers;
mod response;
mod rtk;
mod thinking;
mod tool_adjacency;
mod tools;

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
    runtime_chat_compatible_request_body(
        body,
        conversations,
        RuntimeProviderBridgeKind::DeepSeek,
        SUPER_DEEPSEEK_DEFAULT_MODEL,
        true,
    )
}

pub(super) fn runtime_chat_compatible_request_body(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
    provider_kind: RuntimeProviderBridgeKind,
    default_model: &str,
    include_reasoning_params: bool,
) -> Result<RuntimeDeepSeekTranslatedRequest> {
    let value: serde_json::Value =
        serde_json::from_slice(body).context("failed to parse Codex Responses request JSON")?;
    let mut request = serde_json::Map::new();
    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default_model);
    let model = runtime_provider_canonical_model(provider_kind, model);
    let stream = value
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    request.insert("model".to_string(), serde_json::Value::String(model));
    request.insert("stream".to_string(), serde_json::Value::Bool(stream));
    let thinking_enabled = runtime_deepseek_thinking_enabled(&value);
    let mut messages = runtime_deepseek_messages_from_responses_request(&value, conversations)
        .unwrap_or_else(|| {
            vec![serde_json::json!({
                "role": "user",
                "content": "",
            })]
        });
    runtime_deepseek_repair_tool_call_adjacency(&mut messages);
    if messages.is_empty() {
        messages.push(serde_json::json!({
            "role": "user",
            "content": "",
        }));
    }
    if thinking_enabled {
        runtime_deepseek_normalize_thinking_tool_call_messages(&mut messages);
    }
    request.insert(
        "messages".to_string(),
        serde_json::Value::Array(messages.clone()),
    );
    if include_reasoning_params {
        runtime_deepseek_apply_reasoning_from_responses_request(&value, &mut request);
    }
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
            matches!(
                object.get("type").and_then(serde_json::Value::as_str),
                Some("function_call_output" | "mcp_call_output" | "mcp_tool_result")
            )
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
            let call_id = runtime_deepseek_tool_call_id(object);
            if replayed_tool_call_ids.contains(&call_id) {
                return;
            }
            runtime_deepseek_push_chat_tool_call_message(object, call_id, messages);
        }
        Some("function_call_output") => {
            let call_id = runtime_deepseek_tool_output_call_id(object);
            if replayed_tool_output_call_ids.contains(&call_id) {
                return;
            }
            let output = runtime_deepseek_tool_output_text(object);
            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": output,
            }));
        }
        Some("mcp_call") => {
            let call_id = runtime_deepseek_tool_call_id(object);
            if !replayed_tool_call_ids.contains(&call_id) {
                runtime_deepseek_push_chat_tool_call_message(object, call_id.clone(), messages);
            }
            if runtime_deepseek_mcp_call_has_result(object)
                && !replayed_tool_output_call_ids.contains(&call_id)
            {
                messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": runtime_deepseek_tool_output_text(object),
                }));
            }
        }
        Some("mcp_tool_result") | Some("mcp_call_output") => {
            let call_id = runtime_deepseek_tool_output_call_id(object);
            if replayed_tool_output_call_ids.contains(&call_id) {
                return;
            }
            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": runtime_deepseek_tool_output_text(object),
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

fn runtime_deepseek_tool_call_id(object: &serde_json::Map<String, serde_json::Value>) -> String {
    runtime_deepseek_json_string(object, &["call_id", "tool_call_id", "id"])
        .unwrap_or_else(|| "call_0".to_string())
}

fn runtime_deepseek_tool_output_call_id(
    object: &serde_json::Map<String, serde_json::Value>,
) -> String {
    runtime_deepseek_json_string(object, &["call_id", "tool_call_id", "id"])
        .unwrap_or_else(|| "call_0".to_string())
}

fn runtime_deepseek_push_chat_tool_call_message(
    object: &serde_json::Map<String, serde_json::Value>,
    call_id: String,
    messages: &mut Vec<serde_json::Value>,
) {
    let name = runtime_deepseek_tool_call_name(object);
    let arguments = runtime_deepseek_tool_call_arguments(object);
    let mut tool_call = serde_json::json!({
        "id": call_id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
    });
    if let Some(signature) = runtime_deepseek_tool_call_thought_signature(object) {
        tool_call["gemini_thought_signature"] = serde_json::Value::String(signature);
    }
    messages.push(serde_json::json!({
        "role": "assistant",
        "content": "",
        "tool_calls": [tool_call],
    }));
}

fn runtime_deepseek_tool_call_thought_signature(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    runtime_deepseek_json_string(
        object,
        &[
            "gemini_thought_signature",
            "thought_signature",
            "thoughtSignature",
        ],
    )
    .or_else(|| {
        runtime_deepseek_json_string_at_path(
            object,
            &["provider_specific_fields", "thought_signature"],
        )
    })
    .filter(|signature| !signature.trim().is_empty())
}

fn runtime_deepseek_tool_call_name(object: &serde_json::Map<String, serde_json::Value>) -> String {
    runtime_deepseek_json_string(object, &["name", "tool_name"])
        .or_else(|| runtime_deepseek_json_string_at_path(object, &["function", "name"]))
        .unwrap_or_else(|| "tool_call".to_string())
}

fn runtime_deepseek_tool_call_arguments(
    object: &serde_json::Map<String, serde_json::Value>,
) -> String {
    runtime_deepseek_json_string(object, &["arguments", "input"])
        .or_else(|| runtime_deepseek_json_string_at_path(object, &["function", "arguments"]))
        .or_else(|| {
            object
                .get("arguments")
                .or_else(|| object.get("input"))
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "{}".to_string())
}

fn runtime_deepseek_mcp_call_has_result(
    object: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    object.contains_key("output")
        || object.contains_key("content")
        || object.contains_key("result")
        || object.contains_key("error")
}

fn runtime_deepseek_tool_output_text(
    object: &serde_json::Map<String, serde_json::Value>,
) -> String {
    runtime_deepseek_json_string(object, &["output", "content", "result", "error"])
        .or_else(|| {
            object
                .get("output")
                .or_else(|| object.get("content"))
                .or_else(|| object.get("result"))
                .or_else(|| object.get("error"))
                .map(|value| runtime_deepseek_responses_content_text(Some(value)))
        })
        .unwrap_or_default()
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

#[cfg(test)]
#[path = "deepseek_optional_tools_tests.rs"]
mod deepseek_optional_tools_tests;

#[cfg(test)]
#[path = "deepseek_rewrite_tests.rs"]
mod deepseek_rewrite_tests;
