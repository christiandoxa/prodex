//! Responses input/history conversion helpers for the DeepSeek chat bridge.

use std::collections::BTreeSet;

mod history;
mod push;
mod validation;

pub use self::history::{
    deepseek_provider_core_chat_role, deepseek_provider_core_first_function_call_output_call_id,
    deepseek_provider_core_history_has_system_message,
    deepseek_provider_core_history_has_tool_call, deepseek_provider_core_message_signatures,
    deepseek_provider_core_tool_call_ids, deepseek_provider_core_tool_output_call_ids,
};
use self::push::{
    deepseek_provider_core_input_tool_call_id, deepseek_provider_core_input_tool_output_call_id,
    deepseek_provider_core_input_tool_output_text, deepseek_provider_core_mcp_call_has_result,
    deepseek_provider_core_push_chat_custom_tool_call_message,
    deepseek_provider_core_push_chat_local_shell_call_message,
    deepseek_provider_core_push_chat_tool_call_message,
};
use self::validation::{
    deepseek_provider_core_reject_chat_prefix_marker,
    deepseek_provider_core_validate_input_local_shell_call_item,
    deepseek_provider_core_validate_input_message_role,
    deepseek_provider_core_validate_input_tool_call_item,
    deepseek_provider_core_validate_input_tool_output_item,
    deepseek_provider_core_validate_supported_message_content,
};
use super::deepseek_provider_core_responses_content_text;

pub fn deepseek_provider_core_validate_supported_input_item(
    item: &serde_json::Value,
    gemini_compat: bool,
    provider_label: &str,
) -> Result<(), String> {
    let Some(object) = item.as_object() else {
        return Err(format!("{provider_label} input items must be objects"));
    };
    deepseek_provider_core_reject_chat_prefix_marker(object, provider_label)?;
    match object.get("type").and_then(serde_json::Value::as_str) {
        Some("message") => {
            deepseek_provider_core_validate_input_message_role(object, provider_label)?;
            deepseek_provider_core_validate_supported_message_content(
                object.get("content"),
                gemini_compat,
                provider_label,
            )
        }
        Some("function_call" | "custom_tool_call" | "mcp_call") => {
            deepseek_provider_core_validate_input_tool_call_item(object, provider_label)
        }
        Some("local_shell_call") => {
            deepseek_provider_core_validate_input_local_shell_call_item(object, provider_label)
        }
        Some(
            "function_call_output"
            | "custom_tool_call_output"
            | "mcp_tool_result"
            | "mcp_call_output",
        ) => deepseek_provider_core_validate_input_tool_output_item(object, provider_label),
        Some(other) => Err(format!(
            "{provider_label} input item type `{other}` is not supported by this Responses adapter"
        )),
        None => Ok(()),
    }
}

pub fn deepseek_provider_core_system_message(content: &str) -> serde_json::Value {
    serde_json::json!({
        "role": "system",
        "content": content,
    })
}

pub fn deepseek_provider_core_user_message(content: &str) -> serde_json::Value {
    serde_json::json!({
        "role": "user",
        "content": content,
    })
}

pub fn deepseek_provider_core_push_message_from_responses_item(
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
        Some("message") | None
            if object.contains_key("role")
                && object.contains_key("content")
                && !object.contains_key("call_id")
                && !object.contains_key("tool_call_id") =>
        {
            let role = object
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("user");
            let role = deepseek_provider_core_chat_role(role);
            let text = deepseek_provider_core_responses_content_text(object.get("content"));
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
            let call_id = deepseek_provider_core_input_tool_call_id(object);
            if replayed_tool_call_ids.contains(&call_id) {
                return;
            }
            deepseek_provider_core_push_chat_tool_call_message(object, call_id, messages);
        }
        Some("custom_tool_call") => {
            let call_id = deepseek_provider_core_input_tool_call_id(object);
            if replayed_tool_call_ids.contains(&call_id) {
                return;
            }
            deepseek_provider_core_push_chat_custom_tool_call_message(object, call_id, messages);
        }
        Some("local_shell_call") => {
            let call_id = deepseek_provider_core_input_tool_call_id(object);
            if replayed_tool_call_ids.contains(&call_id) {
                return;
            }
            deepseek_provider_core_push_chat_local_shell_call_message(object, call_id, messages);
        }
        Some("function_call_output") | Some("custom_tool_call_output") => {
            let call_id = deepseek_provider_core_input_tool_output_call_id(object);
            if replayed_tool_output_call_ids.contains(&call_id) {
                return;
            }
            let output = deepseek_provider_core_input_tool_output_text(object);
            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": output,
            }));
        }
        Some("mcp_call") => {
            let call_id = deepseek_provider_core_input_tool_call_id(object);
            if !replayed_tool_call_ids.contains(&call_id) {
                deepseek_provider_core_push_chat_tool_call_message(
                    object,
                    call_id.clone(),
                    messages,
                );
            }
            if deepseek_provider_core_mcp_call_has_result(object)
                && !replayed_tool_output_call_ids.contains(&call_id)
            {
                messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": deepseek_provider_core_input_tool_output_text(object),
                }));
            }
        }
        Some("mcp_tool_result") | Some("mcp_call_output") => {
            let call_id = deepseek_provider_core_input_tool_output_call_id(object);
            if replayed_tool_output_call_ids.contains(&call_id) {
                return;
            }
            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": deepseek_provider_core_input_tool_output_text(object),
            }));
        }
        Some(_) => {}
        None => {}
    }
}
