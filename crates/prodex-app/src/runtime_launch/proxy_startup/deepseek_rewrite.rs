use self::json_helpers::{runtime_deepseek_json_string, runtime_deepseek_json_string_at_path};
#[cfg(test)]
pub(in crate::runtime_launch::proxy_startup) use self::response::{
    runtime_deepseek_chat_assistant_messages_from_response_value,
    runtime_deepseek_responses_value_from_chat_value,
};
pub(in crate::runtime_launch::proxy_startup) use self::response::{
    runtime_deepseek_chat_buffered_response_parts,
    runtime_deepseek_chat_tool_call_thought_signature, runtime_deepseek_created_at,
    runtime_deepseek_merge_response_metadata,
    runtime_deepseek_normalize_assistant_tool_call_content, runtime_deepseek_responses_usage,
    runtime_deepseek_store_conversation, runtime_deepseek_take_pending_messages,
};
pub(super) use self::rtk::runtime_deepseek_rtk_wrapped_tool_arguments;
use self::thinking::runtime_deepseek_normalize_thinking_tool_call_messages;
use self::tool_adjacency::runtime_deepseek_repair_tool_call_adjacency;
use self::tools::{
    runtime_deepseek_tool_choice_from_responses_request,
    runtime_deepseek_tools_from_responses_request,
    runtime_deepseek_web_search_options_from_responses_request,
};
use super::deepseek_content::{
    runtime_deepseek_responses_content_text, runtime_deepseek_responses_content_text_value,
};
use super::deepseek_reasoning::{
    runtime_deepseek_apply_reasoning_from_responses_request, runtime_deepseek_thinking_enabled,
};
pub(super) use super::deepseek_sse_reader::RuntimeDeepSeekChatSseReader;
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
pub(super) type RuntimeDeepSeekPendingMessages =
    Arc<Mutex<BTreeMap<u64, RuntimeDeepSeekPendingRequest>>>;

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeDeepSeekPendingRequest {
    pub(super) messages: Vec<serde_json::Value>,
    pub(super) response_metadata: Option<serde_json::Value>,
}

#[derive(Debug)]
pub(crate) struct RuntimeDeepSeekTranslatedRequest {
    pub(crate) body: Vec<u8>,
    pub(crate) messages: Vec<serde_json::Value>,
    pub(crate) response_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RuntimeDeepSeekRewriteOptions {
    pub(crate) strict_tools: bool,
    pub(crate) web_search_mode: RuntimeDeepSeekWebSearchMode,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum RuntimeDeepSeekWebSearchMode {
    #[default]
    Auto,
    Off,
    OpenAiChat,
    Anthropic,
    FunctionProxy,
}

pub(super) fn runtime_deepseek_chat_request_body(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
) -> Result<RuntimeDeepSeekTranslatedRequest> {
    runtime_deepseek_chat_request_body_with_options(
        body,
        conversations,
        RuntimeDeepSeekRewriteOptions::default(),
    )
}

pub(super) fn runtime_deepseek_chat_request_body_with_options(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
    options: RuntimeDeepSeekRewriteOptions,
) -> Result<RuntimeDeepSeekTranslatedRequest> {
    runtime_chat_compatible_request_body(
        body,
        conversations,
        RuntimeProviderBridgeKind::DeepSeek,
        SUPER_DEEPSEEK_DEFAULT_MODEL,
        true,
        options,
    )
}

pub(super) fn runtime_chat_compatible_request_body(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
    provider_kind: RuntimeProviderBridgeKind,
    default_model: &str,
    include_reasoning_params: bool,
    options: RuntimeDeepSeekRewriteOptions,
) -> Result<RuntimeDeepSeekTranslatedRequest> {
    let value: serde_json::Value =
        serde_json::from_slice(body).context("failed to parse Codex Responses request JSON")?;
    runtime_deepseek_reject_beta_completion_fields(&value)?;
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
    let mut messages = runtime_deepseek_messages_from_responses_request(&value, conversations)?
        .unwrap_or_else(|| {
            vec![serde_json::json!({
                "role": "user",
                "content": "",
            })]
        });
    runtime_deepseek_repair_tool_call_adjacency(&mut messages);
    if provider_kind == RuntimeProviderBridgeKind::Gemini {
        runtime_gemini_openai_preserve_tool_call_signatures(&mut messages);
    }
    if messages.is_empty() {
        messages.push(serde_json::json!({
            "role": "user",
            "content": "",
        }));
    }
    if thinking_enabled {
        runtime_deepseek_normalize_thinking_tool_call_messages(&mut messages);
    }
    let response_format = runtime_deepseek_response_format_from_responses_request(&value);
    let response_metadata = runtime_deepseek_response_metadata_from_responses_request(&value);
    if response_format.is_some() {
        runtime_deepseek_ensure_json_prompt_instruction(&mut messages);
    }
    request.insert(
        "messages".to_string(),
        serde_json::Value::Array(messages.clone()),
    );
    if include_reasoning_params {
        runtime_deepseek_apply_reasoning_from_responses_request(
            &value,
            &mut request,
            provider_kind,
        );
    }
    let mut tool_names = BTreeSet::new();
    if let Some(tools) = runtime_deepseek_tools_from_responses_request(&value) {
        let tools = runtime_deepseek_dedup_and_validate_function_tools(tools, options)?;
        if tools.len() > 128 {
            anyhow::bail!("DeepSeek supports at most 128 function tools");
        }
        tool_names.extend(tools.iter().filter_map(runtime_deepseek_function_tool_name));
        request.insert("tools".to_string(), serde_json::Value::Array(tools));
    }
    if let Some(web_search_options) =
        runtime_deepseek_web_search_options_from_responses_request(&value)
    {
        runtime_deepseek_apply_web_search_mode(&mut request, web_search_options, options)?;
    }
    if let Some(tool_choice) =
        runtime_deepseek_tool_choice_from_responses_request(&value, thinking_enabled)
    {
        runtime_deepseek_validate_tool_choice_name(&tool_choice)?;
        runtime_deepseek_validate_tool_choice_target(&tool_choice, &tool_names)?;
        request.insert("tool_choice".to_string(), tool_choice);
    }
    for (from, to) in [
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("max_output_tokens", "max_tokens"),
        ("max_tokens", "max_tokens"),
        ("logprobs", "logprobs"),
    ] {
        if let Some(next) = value.get(from) {
            request.insert(to.to_string(), next.clone());
        }
    }
    runtime_deepseek_reject_unsupported_request_fields(&value)?;
    if let Some(stop) = runtime_deepseek_stop_from_responses_request(&value)? {
        request.insert("stop".to_string(), stop);
    }
    if let Some(top_logprobs) = runtime_deepseek_top_logprobs_from_responses_request(&value)? {
        request.insert("top_logprobs".to_string(), top_logprobs);
    }
    if stream {
        request.insert(
            "stream_options".to_string(),
            serde_json::json!({"include_usage": true}),
        );
    }
    if let Some(response_format) = response_format {
        request.insert("response_format".to_string(), response_format);
    }
    if let Some(user_id) = runtime_deepseek_user_id_from_responses_request(&value)? {
        request.insert("user_id".to_string(), serde_json::Value::String(user_id));
    }
    let body = serde_json::to_vec(&serde_json::Value::Object(request))
        .context("failed to serialize DeepSeek chat request JSON")?;
    Ok(RuntimeDeepSeekTranslatedRequest {
        body,
        messages,
        response_metadata,
    })
}

fn runtime_gemini_openai_preserve_tool_call_signatures(messages: &mut [serde_json::Value]) {
    for message in messages {
        let Some(tool_calls) = message
            .get_mut("tool_calls")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        for tool_call in tool_calls {
            let Some(object) = tool_call.as_object_mut() else {
                continue;
            };
            let Some(signature) = runtime_deepseek_tool_call_thought_signature(object) else {
                continue;
            };
            object.remove("gemini_thought_signature");
            object.remove("thought_signature");
            object.remove("thoughtSignature");
            object.insert(
                "extra_content".to_string(),
                serde_json::json!({
                    "google": {
                        "thought_signature": signature,
                    }
                }),
            );
        }
    }
}

fn runtime_deepseek_messages_from_responses_request(
    value: &serde_json::Value,
    conversations: &RuntimeDeepSeekConversationStore,
) -> Result<Option<Vec<serde_json::Value>>> {
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
                runtime_deepseek_validate_supported_input_item(item)?;
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
        Ok(None)
    } else {
        Ok(Some(messages))
    }
}

fn runtime_deepseek_validate_supported_input_item(item: &serde_json::Value) -> Result<()> {
    let Some(object) = item.as_object() else {
        return Ok(());
    };
    runtime_deepseek_reject_chat_prefix_marker(object)?;
    if object.get("type").and_then(serde_json::Value::as_str) != Some("message") {
        return Ok(());
    }
    runtime_deepseek_validate_supported_message_content(object.get("content"))
}

fn runtime_deepseek_validate_supported_message_content(
    content: Option<&serde_json::Value>,
) -> Result<()> {
    match content {
        Some(serde_json::Value::Array(parts)) => {
            for part in parts {
                runtime_deepseek_validate_supported_content_part(part)?;
            }
        }
        Some(content @ serde_json::Value::Object(_)) => {
            runtime_deepseek_validate_supported_content_part(content)?;
        }
        _ => {}
    }
    Ok(())
}

fn runtime_deepseek_validate_supported_content_part(part: &serde_json::Value) -> Result<()> {
    let Some(object) = part.as_object() else {
        return Ok(());
    };
    runtime_deepseek_reject_chat_prefix_marker(object)?;
    if object
        .get("text")
        .or_else(|| object.get("input_text"))
        .or_else(|| object.get("output_text"))
        .and_then(serde_json::Value::as_str)
        .is_some()
    {
        return Ok(());
    }
    let Some(part_type) = object.get("type").and_then(serde_json::Value::as_str) else {
        return Ok(());
    };
    if matches!(part_type, "input_text" | "output_text" | "text") {
        return Ok(());
    }
    anyhow::bail!(
        "DeepSeek text-only adapter does not support message content part type `{part_type}`"
    );
}

fn runtime_deepseek_reject_chat_prefix_marker(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    if object.get("prefix").is_some() {
        anyhow::bail!(
            "DeepSeek chat prefix completion requires the beta chat endpoint, which this Responses adapter does not enable yet"
        );
    }
    Ok(())
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
                Some(
                    "function_call_output"
                        | "custom_tool_call_output"
                        | "mcp_call_output"
                        | "mcp_tool_result",
                )
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
        Some("custom_tool_call") => {
            let call_id = runtime_deepseek_tool_call_id(object);
            if replayed_tool_call_ids.contains(&call_id) {
                return;
            }
            runtime_deepseek_push_chat_custom_tool_call_message(object, call_id, messages);
        }
        Some("local_shell_call") => {
            let call_id = runtime_deepseek_tool_call_id(object);
            if replayed_tool_call_ids.contains(&call_id) {
                return;
            }
            runtime_deepseek_push_chat_local_shell_call_message(object, call_id, messages);
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
        Some("custom_tool_call_output") => {
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

fn runtime_deepseek_push_chat_custom_tool_call_message(
    object: &serde_json::Map<String, serde_json::Value>,
    call_id: String,
    messages: &mut Vec<serde_json::Value>,
) {
    let name = runtime_deepseek_tool_call_name(object);
    let input = runtime_deepseek_json_string(object, &["input"])
        .or_else(|| {
            object
                .get("input")
                .map(runtime_deepseek_responses_content_text_value)
        })
        .unwrap_or_default();
    let arguments = serde_json::to_string(&serde_json::json!({ "input": input }))
        .unwrap_or_else(|_| "{\"input\":\"\"}".to_string());
    runtime_deepseek_push_chat_tool_call_message_with_arguments(call_id, name, arguments, messages);
}

fn runtime_deepseek_push_chat_local_shell_call_message(
    object: &serde_json::Map<String, serde_json::Value>,
    call_id: String,
    messages: &mut Vec<serde_json::Value>,
) {
    let command = object
        .get("action")
        .and_then(|action| action.get("command"))
        .and_then(|command| {
            command.as_array().map(|parts| {
                parts
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
        })
        .filter(|command| !command.trim().is_empty())
        .or_else(|| runtime_deepseek_json_string(object, &["command"]))
        .unwrap_or_default();
    let mut shell_arguments = serde_json::Map::new();
    shell_arguments.insert("command".to_string(), serde_json::Value::String(command));
    runtime_deepseek_copy_shell_argument(object, &mut shell_arguments, "cwd");
    runtime_deepseek_copy_shell_argument(object, &mut shell_arguments, "timeout");
    runtime_deepseek_copy_shell_argument(object, &mut shell_arguments, "env");
    let arguments = serde_json::to_string(&serde_json::Value::Object(shell_arguments))
        .unwrap_or_else(|_| "{\"command\":\"\"}".to_string());
    runtime_deepseek_push_chat_tool_call_message_with_arguments(
        call_id,
        "shell_command".to_string(),
        arguments,
        messages,
    );
}

fn runtime_deepseek_copy_shell_argument(
    object: &serde_json::Map<String, serde_json::Value>,
    arguments: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
) {
    if let Some(value) = object
        .get(key)
        .or_else(|| object.get("action").and_then(|action| action.get(key)))
    {
        arguments.insert(key.to_string(), value.clone());
    }
}

fn runtime_deepseek_push_chat_tool_call_message_with_arguments(
    call_id: String,
    name: String,
    arguments: String,
    messages: &mut Vec<serde_json::Value>,
) {
    let tool_call = serde_json::json!({
        "id": call_id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
    });
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
    .or_else(|| {
        runtime_deepseek_json_string_at_path(
            object,
            &["extra_content", "google", "thought_signature"],
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

#[cfg(test)]
#[path = "deepseek_optional_tools_tests.rs"]
mod deepseek_optional_tools_tests;

#[cfg(test)]
#[path = "deepseek_provider_tool_tests.rs"]
mod deepseek_provider_tool_tests;

#[cfg(test)]
#[path = "deepseek_rewrite_rtk_tests.rs"]
mod deepseek_rewrite_rtk_tests;

#[cfg(test)]
#[path = "deepseek_rewrite_tests.rs"]
mod deepseek_rewrite_tests;

fn runtime_deepseek_stop_from_responses_request(
    value: &serde_json::Value,
) -> Result<Option<serde_json::Value>> {
    let Some(stop) = value
        .get("stop")
        .or_else(|| value.get("stop_sequences"))
        .or_else(|| value.get("stopSequences"))
    else {
        return Ok(None);
    };
    if stop.as_str().is_some() {
        return Ok(Some(stop.clone()));
    }
    let Some(stops) = stop.as_array() else {
        return Ok(Some(stop.clone()));
    };
    if stops.len() > 16 {
        anyhow::bail!("DeepSeek supports at most 16 stop sequences");
    }
    if stops.iter().any(|stop| !stop.is_string()) {
        anyhow::bail!("DeepSeek stop sequences must be strings");
    }
    Ok(Some(stop.clone()))
}

fn runtime_deepseek_top_logprobs_from_responses_request(
    value: &serde_json::Value,
) -> Result<Option<serde_json::Value>> {
    let Some(top_logprobs) = value.get("top_logprobs") else {
        return Ok(None);
    };
    if let Some(count) = top_logprobs.as_u64()
        && count > 20
    {
        anyhow::bail!("DeepSeek top_logprobs must be <= 20");
    }
    if value.get("logprobs").and_then(serde_json::Value::as_bool) != Some(true) {
        anyhow::bail!("DeepSeek top_logprobs requires logprobs=true");
    }
    Ok(Some(top_logprobs.clone()))
}

fn runtime_deepseek_reject_unsupported_request_fields(value: &serde_json::Value) -> Result<()> {
    for field in ["frequency_penalty", "presence_penalty"] {
        if value.get(field).is_some() {
            anyhow::bail!("DeepSeek {field} is deprecated and is not forwarded by Prodex");
        }
    }
    if value
        .get("parallel_tool_calls")
        .and_then(serde_json::Value::as_bool)
        == Some(false)
    {
        anyhow::bail!("DeepSeek does not expose a compatible parallel_tool_calls=false control");
    }
    Ok(())
}

fn runtime_deepseek_reject_beta_completion_fields(value: &serde_json::Value) -> Result<()> {
    if value.get("prefix").is_some() {
        anyhow::bail!(
            "DeepSeek chat prefix completion requires the beta chat endpoint, which this Responses adapter does not enable yet"
        );
    }
    if value.get("suffix").is_some() {
        anyhow::bail!(
            "DeepSeek FIM suffix completion requires the beta /completions endpoint, which this Responses adapter does not enable yet"
        );
    }
    if value.get("prompt").is_some() {
        anyhow::bail!(
            "DeepSeek prompt completions require the beta /completions endpoint, which this Responses adapter does not enable yet"
        );
    }
    Ok(())
}

fn runtime_deepseek_dedup_and_validate_function_tools(
    tools: Vec<serde_json::Value>,
    options: RuntimeDeepSeekRewriteOptions,
) -> Result<Vec<serde_json::Value>> {
    let mut seen_tools = BTreeMap::<String, serde_json::Value>::new();
    let mut deduped = Vec::new();
    for mut tool in tools {
        if let Some(name) = tool
            .get("function")
            .and_then(|function| function.get("name"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
        {
            runtime_deepseek_validate_function_name(&name)?;
            if options.strict_tools {
                runtime_deepseek_apply_strict_function_schema(&mut tool)?;
            }
            if let Some(previous) = seen_tools.get(&name) {
                if previous == &tool {
                    continue;
                }
                let previous_generic = runtime_deepseek_is_generic_function_tool(previous);
                let current_generic = runtime_deepseek_is_generic_function_tool(&tool);
                if previous_generic && !current_generic {
                    deduped.retain(|deduped_tool: &serde_json::Value| {
                        deduped_tool
                            .get("function")
                            .and_then(|function| function.get("name"))
                            .and_then(serde_json::Value::as_str)
                            != Some(name.as_str())
                    });
                } else if current_generic {
                    continue;
                } else {
                    anyhow::bail!(
                        "DeepSeek function tool name `{name}` is duplicated after translation"
                    );
                }
            }
            seen_tools.insert(name, tool.clone());
        }
        deduped.push(tool);
    }
    Ok(deduped)
}

fn runtime_deepseek_apply_strict_function_schema(tool: &mut serde_json::Value) -> Result<()> {
    let Some(function) = tool
        .get_mut("function")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Ok(());
    };
    let name = function
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("function")
        .to_string();
    let parameters = function
        .entry("parameters".to_string())
        .or_insert_with(|| serde_json::json!({"type": "object"}));
    runtime_deepseek_sanitize_strict_schema(parameters, &name)?;
    function.insert("strict".to_string(), serde_json::Value::Bool(true));
    Ok(())
}

fn runtime_deepseek_sanitize_strict_schema(
    schema: &mut serde_json::Value,
    path: &str,
) -> Result<()> {
    let Some(object) = schema.as_object_mut() else {
        anyhow::bail!("DeepSeek strict tool schema `{path}` must be a JSON object");
    };
    runtime_deepseek_reject_strict_schema_keywords(object, path)?;
    if let Some(any_of) = object.get_mut("anyOf") {
        let Some(items) = any_of.as_array_mut() else {
            anyhow::bail!("DeepSeek strict tool schema `{path}.anyOf` must be an array");
        };
        for (index, item) in items.iter_mut().enumerate() {
            runtime_deepseek_sanitize_strict_schema(item, &format!("{path}.anyOf[{index}]"))?;
        }
        return Ok(());
    }
    if let Some(enum_values) = object.get("enum")
        && !enum_values.is_array()
    {
        anyhow::bail!("DeepSeek strict tool schema `{path}.enum` must be an array");
    }
    let schema_type = object
        .entry("type".to_string())
        .or_insert_with(|| serde_json::Value::String("object".to_string()))
        .as_str()
        .ok_or_else(|| {
            anyhow::anyhow!("DeepSeek strict tool schema `{path}.type` must be a string")
        })?;
    if !matches!(
        schema_type,
        "object" | "string" | "number" | "integer" | "boolean" | "array"
    ) {
        anyhow::bail!("DeepSeek strict tool schema `{path}` uses unsupported type `{schema_type}`");
    }
    match schema_type {
        "object" => runtime_deepseek_sanitize_strict_object_schema(object, path),
        "array" => {
            let Some(items) = object.get_mut("items") else {
                anyhow::bail!("DeepSeek strict tool schema `{path}` array requires items");
            };
            runtime_deepseek_sanitize_strict_schema(items, &format!("{path}.items"))
        }
        _ => Ok(()),
    }
}

fn runtime_deepseek_sanitize_strict_object_schema(
    object: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Result<()> {
    let properties = object
        .entry("properties".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let Some(properties) = properties.as_object_mut() else {
        anyhow::bail!("DeepSeek strict tool schema `{path}.properties` must be an object");
    };
    let required = properties.keys().cloned().collect::<Vec<_>>();
    for (name, property) in properties.iter_mut() {
        runtime_deepseek_sanitize_strict_schema(property, &format!("{path}.{name}"))?;
    }
    object.insert("required".to_string(), serde_json::json!(required));
    object.insert(
        "additionalProperties".to_string(),
        serde_json::Value::Bool(false),
    );
    Ok(())
}

fn runtime_deepseek_reject_strict_schema_keywords(
    object: &serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Result<()> {
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "type"
                | "description"
                | "properties"
                | "required"
                | "additionalProperties"
                | "items"
                | "enum"
                | "anyOf"
        ) {
            anyhow::bail!("DeepSeek strict tool schema `{path}` uses unsupported keyword `{key}`");
        }
    }
    Ok(())
}

fn runtime_deepseek_function_tool_name(tool: &serde_json::Value) -> Option<String> {
    tool.get("function")
        .and_then(|function| function.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn runtime_deepseek_is_generic_function_tool(tool: &serde_json::Value) -> bool {
    let Some(parameters) = tool
        .get("function")
        .and_then(|function| function.get("parameters"))
    else {
        return false;
    };
    parameters
        .get("additionalProperties")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
        && parameters.get("properties").is_none()
        && parameters.get("required").is_none()
}

fn runtime_deepseek_validate_tool_choice_name(tool_choice: &serde_json::Value) -> Result<()> {
    if let Some(name) = tool_choice
        .get("function")
        .and_then(|function| function.get("name"))
        .and_then(serde_json::Value::as_str)
    {
        runtime_deepseek_validate_function_name(name)?;
    }
    Ok(())
}

fn runtime_deepseek_validate_tool_choice_target(
    tool_choice: &serde_json::Value,
    tool_names: &BTreeSet<String>,
) -> Result<()> {
    let Some(name) = runtime_deepseek_function_tool_name(tool_choice) else {
        return Ok(());
    };
    if !tool_names.contains(&name) {
        anyhow::bail!(
            "DeepSeek named tool_choice `{name}` does not match any translated function tool"
        );
    }
    Ok(())
}

fn runtime_deepseek_validate_function_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        anyhow::bail!(
            "DeepSeek function tool names must use only letters, numbers, underscores, or dashes and be at most 64 bytes"
        );
    }
    Ok(())
}

fn runtime_deepseek_validate_web_search_options(options: &serde_json::Value) -> Result<()> {
    let Some(object) = options.as_object() else {
        return Ok(());
    };
    if let Some(allowed_domains) = object.get("allowed_domains") {
        let Some(domains) = allowed_domains.as_array() else {
            anyhow::bail!("DeepSeek web_search allowed_domains must be an array of strings");
        };
        if domains.iter().any(|domain| {
            domain
                .as_str()
                .is_none_or(|domain| domain.trim().is_empty())
        }) {
            anyhow::bail!("DeepSeek web_search allowed_domains entries must be non-empty strings");
        }
    }
    if object
        .get("user_location")
        .is_some_and(|location| !location.is_object())
    {
        anyhow::bail!("DeepSeek web_search user_location must be an object");
    }
    Ok(())
}

fn runtime_deepseek_apply_web_search_mode(
    request: &mut serde_json::Map<String, serde_json::Value>,
    web_search_options: serde_json::Value,
    options: RuntimeDeepSeekRewriteOptions,
) -> Result<()> {
    match options.web_search_mode {
        RuntimeDeepSeekWebSearchMode::Auto | RuntimeDeepSeekWebSearchMode::OpenAiChat => {
            runtime_deepseek_validate_web_search_options(&web_search_options)?;
            request.insert("web_search_options".to_string(), web_search_options);
            Ok(())
        }
        RuntimeDeepSeekWebSearchMode::Off => {
            anyhow::bail!(
                "DeepSeek web search mode is off; remove web_search tools or set deepseek.web_search_mode"
            )
        }
        RuntimeDeepSeekWebSearchMode::Anthropic => {
            anyhow::bail!(
                "DeepSeek web search mode `anthropic` requires the DeepSeek Anthropic adapter, which this Responses adapter does not enable yet"
            )
        }
        RuntimeDeepSeekWebSearchMode::FunctionProxy => {
            anyhow::bail!(
                "DeepSeek web search mode `function_proxy` requires a local search backend, which this adapter does not enable yet"
            )
        }
    }
}

fn runtime_deepseek_response_format_from_responses_request(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    let response_format = value
        .get("response_format")
        .or_else(|| value.get("text").and_then(|text| text.get("format")))?;
    let format_type = response_format
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    matches!(
        format_type,
        "json_object" | "json_schema" | "json" | "structured_output"
    )
    .then(|| serde_json::json!({"type": "json_object"}))
}

fn runtime_deepseek_response_metadata_from_responses_request(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    let response_format = value
        .get("response_format")
        .or_else(|| value.get("text").and_then(|text| text.get("format")))?;
    let format_type = response_format
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    matches!(format_type, "json_schema" | "structured_output").then(|| {
        serde_json::json!({
            "deepseek": {
                "degraded_response_format": {
                    "from": format_type,
                    "to": "json_object",
                    "reason": "DeepSeek OpenAI Chat response_format supports json_object but not native JSON Schema enforcement"
                }
            }
        })
    })
}

fn runtime_deepseek_ensure_json_prompt_instruction(messages: &mut Vec<serde_json::Value>) {
    if messages
        .iter()
        .any(runtime_deepseek_message_has_json_guidance)
    {
        return;
    }
    messages.insert(
        0,
        serde_json::json!({
            "role": "system",
            "content": "Respond with valid JSON only.",
        }),
    );
}

fn runtime_deepseek_message_has_json_guidance(message: &serde_json::Value) -> bool {
    if !matches!(
        message.get("role").and_then(serde_json::Value::as_str),
        Some("system" | "user")
    ) {
        return false;
    }
    message
        .get("content")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|content| content.to_ascii_lowercase().contains("json"))
}

fn runtime_deepseek_user_id_from_responses_request(
    value: &serde_json::Value,
) -> Result<Option<String>> {
    let Some(user_id) = value
        .get("user_id")
        .or_else(|| value.get("user"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|user| !user.is_empty())
    else {
        return Ok(None);
    };
    if user_id.len() > 512
        || !user_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        anyhow::bail!(
            "DeepSeek user_id must use only letters, numbers, underscores, or dashes and be at most 512 bytes"
        );
    }
    Ok(Some(user_id.to_string()))
}
