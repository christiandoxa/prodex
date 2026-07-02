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

#[cfg(test)]
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
    let gemini_compat = provider_kind == RuntimeProviderBridgeKind::Gemini;
    let value: serde_json::Value =
        serde_json::from_slice(body).context("failed to parse Codex Responses request JSON")?;
    runtime_deepseek_reject_beta_completion_fields(&value)?;
    runtime_deepseek_validate_top_level_request_shape(&value)?;
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
    let mut messages =
        runtime_deepseek_messages_from_responses_request(&value, conversations, gemini_compat)?
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
    let response_format = runtime_deepseek_response_format_from_responses_request(&value)?;
    let mut response_metadata = runtime_deepseek_response_metadata_from_responses_request(&value)?;
    runtime_deepseek_note_thinking_tool_choice_omission(
        &value,
        thinking_enabled,
        &mut response_metadata,
    );
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
        )?;
    }
    let mut tool_names = BTreeSet::new();
    runtime_deepseek_validate_tools_shape(&value, gemini_compat)?;
    if let Some(tools) = runtime_deepseek_tools_from_responses_request(&value) {
        let tools = runtime_deepseek_dedup_and_validate_function_tools(tools, options)?;
        if tools.len() > 128 {
            anyhow::bail!("DeepSeek supports at most 128 function tools");
        }
        tool_names.extend(tools.iter().filter_map(runtime_deepseek_function_tool_name));
        request.insert("tools".to_string(), serde_json::Value::Array(tools));
    }
    if !gemini_compat
        && let Some(web_search_options) =
            runtime_deepseek_web_search_options_from_responses_request(&value)?
    {
        runtime_deepseek_apply_web_search_mode(&mut request, web_search_options, options)?;
    }
    runtime_deepseek_validate_tool_choice_shape(&value, thinking_enabled)?;
    if let Some(tool_choice) =
        runtime_deepseek_tool_choice_from_responses_request(&value, thinking_enabled)
    {
        runtime_deepseek_validate_tool_choice_name(&tool_choice)?;
        runtime_deepseek_validate_tool_choice_target(&tool_choice, &tool_names)?;
        request.insert("tool_choice".to_string(), tool_choice);
    }
    runtime_deepseek_insert_primitive_request_fields(&value, &mut request)?;
    if !gemini_compat {
        runtime_deepseek_reject_unsupported_request_fields(&value)?;
    }
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

fn runtime_deepseek_validate_top_level_request_shape(value: &serde_json::Value) -> Result<()> {
    if !value.is_object() {
        anyhow::bail!("DeepSeek request body must be a JSON object");
    }
    if let Some(model) = value.get("model") {
        let Some(model) = model.as_str() else {
            anyhow::bail!("DeepSeek model must be a string");
        };
        if model.trim().is_empty() {
            anyhow::bail!("DeepSeek model must not be empty");
        }
    }
    if let Some(stream) = value.get("stream")
        && !stream.is_boolean()
    {
        anyhow::bail!("DeepSeek stream must be a boolean");
    }
    if let Some(instructions) = value.get("instructions")
        && !instructions.is_string()
    {
        anyhow::bail!("DeepSeek instructions must be a string");
    }
    if let Some(previous_response_id) = value.get("previous_response_id")
        && !previous_response_id.is_string()
    {
        anyhow::bail!("DeepSeek previous_response_id must be a string");
    }
    Ok(())
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
    gemini_compat: bool,
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
                runtime_deepseek_validate_supported_input_item(item, gemini_compat)?;
                runtime_deepseek_push_message_from_responses_item(
                    item,
                    &mut messages,
                    &replayed_tool_call_ids,
                    &replayed_tool_output_call_ids,
                    &replayed_message_signatures,
                );
            }
        }
        Some(_) => anyhow::bail!("DeepSeek input must be a string or array of input items"),
        None => {}
    }
    if messages.is_empty() {
        Ok(None)
    } else {
        Ok(Some(messages))
    }
}

fn runtime_deepseek_validate_supported_input_item(
    item: &serde_json::Value,
    gemini_compat: bool,
) -> Result<()> {
    let Some(object) = item.as_object() else {
        anyhow::bail!("DeepSeek input items must be objects");
    };
    runtime_deepseek_reject_chat_prefix_marker(object)?;
    match object.get("type").and_then(serde_json::Value::as_str) {
        Some("message") => {
            runtime_deepseek_validate_input_message_role(object)?;
            runtime_deepseek_validate_supported_message_content(
                object.get("content"),
                gemini_compat,
            )
        }
        Some("function_call" | "custom_tool_call" | "mcp_call") => {
            runtime_deepseek_validate_input_tool_call_item(object)
        }
        Some("local_shell_call") => runtime_deepseek_validate_input_local_shell_call_item(object),
        Some(
            "function_call_output"
            | "custom_tool_call_output"
            | "mcp_tool_result"
            | "mcp_call_output",
        ) => runtime_deepseek_validate_input_tool_output_item(object),
        Some(other) => {
            anyhow::bail!(
                "DeepSeek input item type `{other}` is not supported by this Responses adapter"
            )
        }
        None => Ok(()),
    }
}

fn runtime_deepseek_validate_input_message_role(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let Some(role) = object.get("role") else {
        return Ok(());
    };
    let Some(role) = role.as_str() else {
        anyhow::bail!("DeepSeek message role must be a string");
    };
    if matches!(role, "user" | "assistant" | "system" | "developer") {
        return Ok(());
    }
    anyhow::bail!("DeepSeek message role `{role}` is not supported by this Responses adapter")
}

fn runtime_deepseek_validate_input_tool_call_item(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    runtime_deepseek_require_input_item_call_id(object)?;
    if runtime_deepseek_input_item_tool_name(object).is_none() {
        anyhow::bail!("DeepSeek input tool call items require a function name");
    }
    Ok(())
}

fn runtime_deepseek_validate_input_tool_output_item(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    runtime_deepseek_require_input_item_call_id(object)?;
    if ["output", "content", "result", "error"]
        .iter()
        .any(|key| object.contains_key(*key))
    {
        return Ok(());
    }
    anyhow::bail!("DeepSeek input tool output items require output content")
}

fn runtime_deepseek_validate_input_local_shell_call_item(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    runtime_deepseek_require_input_item_call_id(object)?;
    if let Some(parts) = object
        .get("action")
        .and_then(|action| action.get("command"))
        .and_then(serde_json::Value::as_array)
    {
        if !parts.is_empty()
            && parts
                .iter()
                .all(|part| part.as_str().is_some_and(|part| !part.trim().is_empty()))
        {
            return Ok(());
        }
        anyhow::bail!("DeepSeek local_shell_call action.command must be an array of strings");
    }
    if runtime_deepseek_json_string(object, &["command"])
        .filter(|command| !command.trim().is_empty())
        .is_some()
    {
        return Ok(());
    }
    anyhow::bail!("DeepSeek local_shell_call requires a command");
}

fn runtime_deepseek_require_input_item_call_id(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    if runtime_deepseek_json_string(object, &["call_id", "tool_call_id", "id"])
        .filter(|call_id| !call_id.trim().is_empty())
        .is_some()
    {
        return Ok(());
    }
    anyhow::bail!("DeepSeek input tool items require a call_id")
}

fn runtime_deepseek_input_item_tool_name(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    runtime_deepseek_json_string(object, &["name", "tool_name"])
        .or_else(|| runtime_deepseek_json_string_at_path(object, &["function", "name"]))
        .filter(|name| !name.trim().is_empty())
}

fn runtime_deepseek_validate_supported_message_content(
    content: Option<&serde_json::Value>,
    gemini_compat: bool,
) -> Result<()> {
    match content {
        Some(serde_json::Value::Array(parts)) => {
            for part in parts {
                runtime_deepseek_validate_supported_content_part(part, gemini_compat)?;
            }
        }
        Some(content @ serde_json::Value::Object(_)) => {
            runtime_deepseek_validate_supported_content_part(content, gemini_compat)?;
        }
        Some(serde_json::Value::String(_)) | None => {}
        Some(_) => anyhow::bail!("DeepSeek message content must be a string, object, or array"),
    }
    Ok(())
}

fn runtime_deepseek_validate_supported_content_part(
    part: &serde_json::Value,
    gemini_compat: bool,
) -> Result<()> {
    let Some(object) = part.as_object() else {
        return Ok(());
    };
    runtime_deepseek_reject_chat_prefix_marker(object)?;
    if object.get("cache_control").is_some() {
        anyhow::bail!(
            "DeepSeek per-message cache_control is not supported by this Responses adapter because DeepSeek context caching is automatic"
        );
    }
    let has_text = object
        .get("text")
        .or_else(|| object.get("input_text"))
        .or_else(|| object.get("output_text"))
        .and_then(serde_json::Value::as_str)
        .is_some();
    if has_text {
        return Ok(());
    }
    let Some(part_type) = object.get("type").and_then(serde_json::Value::as_str) else {
        anyhow::bail!(
            "DeepSeek text-only adapter does not support object message content parts without text or type"
        );
    };
    if gemini_compat
        && matches!(
            part_type,
            "input_image"
                | "image_url"
                | "input_file"
                | "file"
                | "media"
                | "input_audio"
                | "input_video"
        )
    {
        return Ok(());
    }
    if matches!(part_type, "input_text" | "output_text" | "text") {
        anyhow::bail!("DeepSeek {part_type} content parts require a text field");
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
        Some("message")
        | None
            if object.contains_key("role")
                && object.contains_key("content")
                && !object.contains_key("call_id")
                && !object.contains_key("tool_call_id") =>
        {
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
        Some(_) => {}
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
        anyhow::bail!("DeepSeek stop must be a string or array of strings");
    };
    if stops.len() > 16 {
        anyhow::bail!("DeepSeek supports at most 16 stop sequences");
    }
    if stops.iter().any(|stop| !stop.is_string()) {
        anyhow::bail!("DeepSeek stop sequences must be strings");
    }
    Ok(Some(stop.clone()))
}

fn runtime_deepseek_insert_primitive_request_fields(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    for field in ["temperature", "top_p"] {
        if let Some(next) = value.get(field) {
            if !next.is_number() {
                anyhow::bail!("DeepSeek {field} must be a number");
            }
            request.insert(field.to_string(), next.clone());
        }
    }
    for field in ["max_output_tokens", "max_tokens", "max_completion_tokens"] {
        if let Some(next) = value.get(field) {
            if next.as_u64().is_none_or(|count| count == 0) {
                anyhow::bail!("DeepSeek {field} must be a positive integer");
            }
            request.insert("max_tokens".to_string(), next.clone());
        }
    }
    if let Some(logprobs) = value.get("logprobs") {
        if !logprobs.is_boolean() {
            anyhow::bail!("DeepSeek logprobs must be a boolean");
        }
        request.insert("logprobs".to_string(), logprobs.clone());
    }
    Ok(())
}

fn runtime_deepseek_top_logprobs_from_responses_request(
    value: &serde_json::Value,
) -> Result<Option<serde_json::Value>> {
    let Some(top_logprobs) = value.get("top_logprobs") else {
        return Ok(None);
    };
    let Some(count) = top_logprobs.as_u64() else {
        anyhow::bail!("DeepSeek top_logprobs must be an integer");
    };
    if count > 20 {
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
    for field in [
        "n",
        "seed",
        "service_tier",
        "prediction",
        "logit_bias",
        "functions",
        "function_call",
    ] {
        if value.get(field).is_some() {
            anyhow::bail!("DeepSeek {field} is not supported by this Responses adapter");
        }
    }
    if let Some(include) = value.get("include")
        && !include.is_array()
    {
        anyhow::bail!("DeepSeek include must be an array");
    }
    if let Some(store) = value.get("store")
        && !store.is_boolean()
    {
        anyhow::bail!("DeepSeek store must be a boolean");
    }
    if let Some(background) = value.get("background") {
        match background.as_bool() {
            Some(false) => {}
            Some(true) => anyhow::bail!(
                "DeepSeek background responses are not supported by this Responses adapter"
            ),
            None => anyhow::bail!("DeepSeek background must be a boolean"),
        }
    }
    if let Some(truncation) = value.get("truncation") {
        match truncation.as_str() {
            Some("disabled") => {}
            Some("auto") => {
                anyhow::bail!("DeepSeek truncation=auto is not supported by this Responses adapter")
            }
            Some(other) => anyhow::bail!("DeepSeek truncation `{other}` is not supported"),
            None => anyhow::bail!("DeepSeek truncation must be a string"),
        }
    }
    if value.get("max_tool_calls").is_some() {
        anyhow::bail!("DeepSeek max_tool_calls is not supported by this Responses adapter");
    }
    if let Some(text) = value.get("text") {
        let Some(text) = text.as_object() else {
            anyhow::bail!("DeepSeek text must be an object");
        };
        for key in text.keys() {
            if key != "format" {
                anyhow::bail!("DeepSeek text.{key} is not supported by this Responses adapter");
            }
        }
    }
    if let Some(parallel_tool_calls) = value.get("parallel_tool_calls") {
        match parallel_tool_calls.as_bool() {
            Some(true) => {}
            Some(false) => {
                anyhow::bail!(
                    "DeepSeek does not expose a compatible parallel_tool_calls=false control"
                );
            }
            None => anyhow::bail!("DeepSeek parallel_tool_calls must be a boolean"),
        }
    }
    if let Some(stream_options) = value.get("stream_options") {
        let Some(stream_options) = stream_options.as_object() else {
            anyhow::bail!("DeepSeek stream_options must be an object");
        };
        if value.get("stream").and_then(serde_json::Value::as_bool) != Some(true) {
            anyhow::bail!("DeepSeek stream_options requires stream=true");
        }
        for key in stream_options.keys() {
            if key != "include_usage" {
                anyhow::bail!("DeepSeek stream_options.{key} is not supported");
            }
        }
        if let Some(include_usage) = stream_options.get("include_usage") {
            match include_usage.as_bool() {
                Some(true) => {}
                Some(false) => {
                    anyhow::bail!(
                        "DeepSeek streaming adapter requires stream_options.include_usage=true"
                    )
                }
                None => anyhow::bail!("DeepSeek stream_options.include_usage must be a boolean"),
            }
        }
    }
    if let Some(modalities) = value.get("modalities") {
        let Some(modalities) = modalities.as_array() else {
            anyhow::bail!("DeepSeek modalities must be an array");
        };
        if modalities
            .iter()
            .any(|modality| modality.as_str() != Some("text"))
        {
            anyhow::bail!(
                "DeepSeek Responses adapter only supports text modality; audio/image/video modalities are not supported"
            );
        }
    }
    if value.get("audio").is_some() {
        anyhow::bail!("DeepSeek Responses adapter does not support audio output");
    }
    Ok(())
}

fn runtime_deepseek_validate_tools_shape(
    value: &serde_json::Value,
    gemini_compat: bool,
) -> Result<()> {
    let Some(tools) = value.get("tools") else {
        return Ok(());
    };
    let Some(tools) = tools.as_array() else {
        anyhow::bail!("DeepSeek tools must be an array");
    };
    if tools.iter().any(|tool| !tool.is_object()) {
        anyhow::bail!("DeepSeek tools entries must be objects");
    }
    for tool in tools {
        let Some(object) = tool.as_object() else {
            continue;
        };
        let Some(tool_type) = object.get("type").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if tool_type == "function"
            || tool_type == "custom"
            || tool_type == "namespace"
            || tool_type == "tool_search"
            || tool_type == "mcp"
            || tool_type == "mcp_toolset"
            || tool_type.starts_with("mcp")
            || tool_type == "web_search"
            || tool_type == "web_search_preview"
            || tool_type.starts_with("web_search_preview_")
            || (gemini_compat
                && matches!(
                    tool_type,
                    "web_fetch"
                        | "url_context"
                        | "urlContext"
                        | "web_fetch_preview"
                        | "code_interpreter"
                        | "code_execution"
                        | "codeExecution"
                        | "computer"
                        | "computer_use"
                        | "computerUse"
                        | "computer_use_preview"
                ))
            || (gemini_compat
                && (tool_type.starts_with("web_fetch_preview_")
                    || tool_type.starts_with("computer_")))
        {
            runtime_deepseek_validate_optional_string(object, "description", "tool description")?;
            if let Some(function) = object
                .get("function")
                .and_then(serde_json::Value::as_object)
            {
                runtime_deepseek_validate_optional_string(
                    function,
                    "description",
                    "function description",
                )?;
                runtime_deepseek_validate_optional_bool(function, "strict", "function strict")?;
            }
            runtime_deepseek_validate_optional_bool(object, "strict", "tool strict")?;
            if matches!(tool_type, "function" | "custom")
                && runtime_deepseek_tool_name_from_tool_object(object).is_none()
            {
                anyhow::bail!("DeepSeek {tool_type} tools require a name");
            }
            if tool_type == "custom"
                && object.get("strict").and_then(serde_json::Value::as_bool) == Some(true)
            {
                anyhow::bail!(
                    "DeepSeek custom tools cannot preserve strict=true through the function wrapper"
                );
            }
            if tool_type == "custom" {
                runtime_deepseek_validate_custom_tool_format_shape(object)?;
            }
            if tool_type == "namespace" {
                runtime_deepseek_validate_namespace_tool_shape(object)?;
            }
            if matches!(tool_type, "mcp" | "mcp_toolset") {
                runtime_deepseek_validate_mcp_toolset_shape(object)?;
            }
            if tool_type.starts_with("mcp") && !matches!(tool_type, "mcp" | "mcp_toolset") {
                runtime_deepseek_validate_mcp_function_tool_shape(object)?;
            }
            continue;
        }
        anyhow::bail!("DeepSeek tool type `{tool_type}` is not supported");
    }
    Ok(())
}

fn runtime_deepseek_validate_optional_string(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    label: &str,
) -> Result<()> {
    if let Some(value) = object.get(key)
        && !value.is_string()
    {
        anyhow::bail!("DeepSeek {label} must be a string");
    }
    Ok(())
}

fn runtime_deepseek_validate_optional_bool(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    label: &str,
) -> Result<()> {
    if let Some(value) = object.get(key)
        && !value.is_boolean()
    {
        anyhow::bail!("DeepSeek {label} must be a boolean");
    }
    Ok(())
}

fn runtime_deepseek_validate_custom_tool_format_shape(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let Some(format) = object.get("format") else {
        return Ok(());
    };
    let Some(format) = format.as_object() else {
        anyhow::bail!("DeepSeek custom tool format must be an object");
    };
    if let Some(format_type) = format.get("type")
        && !format_type.is_string()
    {
        anyhow::bail!("DeepSeek custom tool format.type must be a string");
    }
    Ok(())
}

fn runtime_deepseek_validate_namespace_tool_shape(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let Some(namespace) =
        runtime_deepseek_json_string(object, &["name"]).filter(|name| !name.trim().is_empty())
    else {
        anyhow::bail!("DeepSeek namespace tools require a name");
    };
    let Some(tools) = object.get("tools").and_then(serde_json::Value::as_array) else {
        anyhow::bail!("DeepSeek namespace tool `{namespace}` requires a tools array");
    };
    if tools.is_empty() {
        anyhow::bail!("DeepSeek namespace tool `{namespace}` requires at least one tool");
    }
    for tool in tools {
        let Some(tool) = tool.as_object() else {
            anyhow::bail!("DeepSeek namespace tool `{namespace}` entries must be objects");
        };
        runtime_deepseek_validate_optional_string(
            tool,
            "description",
            "namespace function description",
        )?;
        runtime_deepseek_validate_optional_bool(tool, "strict", "namespace function strict")?;
        if tool.get("type").and_then(serde_json::Value::as_str) != Some("function") {
            anyhow::bail!("DeepSeek namespace tool `{namespace}` entries must be function tools");
        }
        if runtime_deepseek_json_string(tool, &["name"])
            .filter(|name| !name.trim().is_empty())
            .is_none()
        {
            anyhow::bail!("DeepSeek namespace tool `{namespace}` function entries require a name");
        }
    }
    Ok(())
}

fn runtime_deepseek_validate_mcp_function_tool_shape(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let Some(name) =
        runtime_deepseek_json_string(object, &["name"]).filter(|name| !name.trim().is_empty())
    else {
        anyhow::bail!("DeepSeek MCP function tools require a name");
    };
    let has_schema = [
        "parameters",
        "parametersJsonSchema",
        "input_schema",
        "schema",
    ]
    .iter()
    .any(|key| object.get(*key).is_some());
    if !has_schema {
        anyhow::bail!("DeepSeek MCP function tool `{name}` requires a schema");
    }
    Ok(())
}

fn runtime_deepseek_validate_mcp_toolset_shape(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let declares_tools = object.contains_key("allowed_tools") || object.contains_key("configs");
    if !declares_tools {
        return Ok(());
    }
    let Some(server_name) = runtime_deepseek_json_string(
        object,
        &["mcp_server_name", "server_label", "server_name", "name"],
    )
    .filter(|server_name| !server_name.trim().is_empty()) else {
        anyhow::bail!("DeepSeek MCP toolsets require a server name");
    };
    let allowed_count = object
        .get("allowed_tools")
        .and_then(serde_json::Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .count()
        })
        .unwrap_or(0);
    let config_count = object
        .get("configs")
        .and_then(serde_json::Value::as_object)
        .map(|configs| {
            let default_enabled = object
                .get("default_config")
                .and_then(serde_json::Value::as_object)
                .and_then(|config| config.get("enabled"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            configs
                .iter()
                .filter(|(_, config)| {
                    config
                        .as_object()
                        .and_then(|config| config.get("enabled"))
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(default_enabled)
                })
                .count()
        })
        .unwrap_or(0);
    if allowed_count == 0 && config_count == 0 {
        anyhow::bail!(
            "DeepSeek MCP toolset `{server_name}` requires allowed_tools or enabled configs"
        );
    }
    Ok(())
}

fn runtime_deepseek_tool_name_from_tool_object(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    runtime_deepseek_json_string(object, &["name"])
        .or_else(|| {
            object
                .get("function")
                .and_then(serde_json::Value::as_object)
                .and_then(|function| runtime_deepseek_json_string(function, &["name"]))
        })
        .filter(|name| !name.trim().is_empty())
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
            runtime_deepseek_validate_function_parameters(&tool, &name)?;
            if !options.strict_tools
                && tool
                    .get("function")
                    .and_then(|function| function.get("strict"))
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
            {
                anyhow::bail!(
                    "DeepSeek strict function tool `{name}` requires deepseek.strict_tools=true"
                );
            }
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

fn runtime_deepseek_validate_function_parameters(
    tool: &serde_json::Value,
    name: &str,
) -> Result<()> {
    if let Some(parameters) = tool
        .get("function")
        .and_then(|function| function.get("parameters"))
        && !parameters.is_object()
    {
        anyhow::bail!("DeepSeek function tool `{name}` parameters must be an object");
    }
    Ok(())
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

fn runtime_deepseek_validate_tool_choice_shape(
    value: &serde_json::Value,
    thinking_enabled: bool,
) -> Result<()> {
    if thinking_enabled {
        return Ok(());
    }
    let Some(choice) = value.get("tool_choice") else {
        return Ok(());
    };
    if let Some(choice) = choice.as_str() {
        if matches!(choice, "auto" | "none" | "required") {
            return Ok(());
        }
        anyhow::bail!("DeepSeek tool_choice string `{choice}` is not supported");
    }
    let Some(object) = choice.as_object() else {
        anyhow::bail!("DeepSeek tool_choice must be a string or object");
    };
    let choice_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if choice_type == "function" || choice_type.starts_with("mcp") {
        if runtime_deepseek_json_string(object, &["name"])
            .or_else(|| {
                object
                    .get("function")
                    .and_then(serde_json::Value::as_object)
                    .and_then(|function| runtime_deepseek_json_string(function, &["name"]))
            })
            .filter(|name| !name.trim().is_empty())
            .is_none()
        {
            anyhow::bail!("DeepSeek named tool_choice requires a function name");
        }
        return Ok(());
    }
    anyhow::bail!("DeepSeek tool_choice type `{choice_type}` is not supported");
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
        anyhow::bail!("DeepSeek web_search_options must be an object");
    };
    if let Some(size) = object.get("search_context_size") {
        let Some(size) = size.as_str() else {
            anyhow::bail!("DeepSeek web_search search_context_size must be low, medium, or high");
        };
        if !matches!(size, "low" | "medium" | "high") {
            anyhow::bail!("DeepSeek web_search search_context_size must be low, medium, or high");
        }
    }
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
        RuntimeDeepSeekWebSearchMode::Auto => {
            let _ = request;
            let _ = web_search_options;
            anyhow::bail!(
                "DeepSeek web search auto mode has no documented native OpenAI Chat route yet; set deepseek.web_search_mode=openai_chat for best-effort forwarding"
            )
        }
        RuntimeDeepSeekWebSearchMode::OpenAiChat => {
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
) -> Result<Option<serde_json::Value>> {
    let response_format = value
        .get("response_format")
        .or_else(|| value.get("text").and_then(|text| text.get("format")));
    let Some(response_format) = response_format else {
        return Ok(None);
    };
    let format_type = response_format
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match format_type {
        "json_object" | "json_schema" | "json" | "structured_output" => {
            Ok(Some(serde_json::json!({"type": "json_object"})))
        }
        "text" => Ok(None),
        "" => anyhow::bail!("DeepSeek response_format must include a type"),
        other => anyhow::bail!("DeepSeek response_format type `{other}` is not supported"),
    }
}

fn runtime_deepseek_response_metadata_from_responses_request(
    value: &serde_json::Value,
) -> Result<Option<serde_json::Value>> {
    let mut metadata = match value.get("metadata") {
        Some(metadata) => metadata
            .as_object()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("DeepSeek request metadata must be an object"))?,
        None => serde_json::Map::new(),
    };
    if metadata
        .get("deepseek")
        .is_some_and(|deepseek| !deepseek.is_object())
    {
        anyhow::bail!("DeepSeek request metadata.deepseek must be an object");
    }
    if let Some(client_metadata) = value.get("client_metadata") {
        let client_metadata = client_metadata
            .as_object()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("DeepSeek client_metadata must be an object"))?;
        metadata.insert(
            "client_metadata".to_string(),
            serde_json::Value::Object(client_metadata),
        );
    }
    if let Some(prompt_cache_key) = value.get("prompt_cache_key") {
        let prompt_cache_key = prompt_cache_key
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("DeepSeek prompt_cache_key must be a string"))?;
        if !prompt_cache_key.trim().is_empty() {
            metadata.insert(
                "prompt_cache_key".to_string(),
                serde_json::Value::String(prompt_cache_key.to_string()),
            );
        }
    }
    if let Some(prompt_cache_retention) = value.get("prompt_cache_retention") {
        if !prompt_cache_retention.is_string() {
            anyhow::bail!("DeepSeek prompt_cache_retention must be a string");
        }
        metadata.insert(
            "prompt_cache_retention".to_string(),
            prompt_cache_retention.clone(),
        );
    }
    let response_format = value
        .get("response_format")
        .or_else(|| value.get("text").and_then(|text| text.get("format")));
    if let Some(response_format) = response_format {
        let format_type = response_format
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if matches!(format_type, "json_schema" | "structured_output") {
            let deepseek = metadata
                .entry("deepseek".to_string())
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
                .ok_or_else(|| {
                    anyhow::anyhow!("DeepSeek request metadata.deepseek must be an object")
                })?;
            deepseek.insert(
                "degraded_response_format".to_string(),
                serde_json::json!({
                    "from": format_type,
                    "to": "json_object",
                    "reason": "DeepSeek OpenAI Chat response_format supports json_object but not native JSON Schema enforcement"
                }),
            );
        }
    }
    Ok((!metadata.is_empty()).then_some(serde_json::Value::Object(metadata)))
}

fn runtime_deepseek_note_thinking_tool_choice_omission(
    value: &serde_json::Value,
    thinking_enabled: bool,
    response_metadata: &mut Option<serde_json::Value>,
) {
    if !thinking_enabled {
        return;
    }
    let Some(tool_choice) = value.get("tool_choice") else {
        return;
    };
    let metadata = response_metadata
        .get_or_insert_with(|| serde_json::json!({}))
        .as_object_mut();
    let Some(metadata) = metadata else {
        return;
    };
    let deepseek = metadata
        .entry("deepseek".to_string())
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut();
    let Some(deepseek) = deepseek else {
        return;
    };
    deepseek.insert(
        "omitted_tool_choice".to_string(),
        serde_json::json!({
            "from": tool_choice,
            "reason": "DeepSeek thinking mode currently rejects explicit tool_choice on the OpenAI Chat route, so Prodex omits it while preserving translated function tools"
        }),
    );
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
        .or_else(|| value.get("safety_identifier"))
    else {
        return Ok(None);
    };
    let Some(user_id) = user_id.as_str() else {
        anyhow::bail!("DeepSeek user_id must be a string");
    };
    let user_id = user_id.trim();
    if user_id.is_empty() {
        return Ok(None);
    }
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
