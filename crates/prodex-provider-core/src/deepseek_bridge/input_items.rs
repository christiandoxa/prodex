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
    deepseek_provider_core_chat_message, deepseek_provider_core_chat_tool_message,
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
    #[cfg(feature = "mojo")]
    {
        let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
            prodex_mojo_core::rich::DeepSeekKernelOperation::SystemMessage,
        );
        input.content = Some(content);
        return deepseek_provider_core_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    serde_json::json!({
        "role": "system",
        "content": content,
    })
}

pub fn deepseek_provider_core_user_message(content: &str) -> serde_json::Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
            prodex_mojo_core::rich::DeepSeekKernelOperation::UserMessage,
        );
        input.content = Some(content);
        return deepseek_provider_core_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
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
            deepseek_provider_core_push_message_item(object, messages, replayed_message_signatures);
        }
        Some("function_call") => {
            deepseek_provider_core_push_tool_call_item(
                object,
                messages,
                replayed_tool_call_ids,
                deepseek_provider_core_push_chat_tool_call_message,
            );
        }
        Some("custom_tool_call") => {
            deepseek_provider_core_push_tool_call_item(
                object,
                messages,
                replayed_tool_call_ids,
                deepseek_provider_core_push_chat_custom_tool_call_message,
            );
        }
        Some("local_shell_call") => {
            deepseek_provider_core_push_tool_call_item(
                object,
                messages,
                replayed_tool_call_ids,
                deepseek_provider_core_push_chat_local_shell_call_message,
            );
        }
        Some("function_call_output") | Some("custom_tool_call_output") => {
            deepseek_provider_core_push_tool_output_item(
                object,
                messages,
                replayed_tool_output_call_ids,
            );
        }
        Some("mcp_call") => {
            deepseek_provider_core_push_mcp_call_item(
                object,
                messages,
                replayed_tool_call_ids,
                replayed_tool_output_call_ids,
            );
        }
        Some("mcp_tool_result") | Some("mcp_call_output") => {
            deepseek_provider_core_push_tool_output_item(
                object,
                messages,
                replayed_tool_output_call_ids,
            );
        }
        Some(_) => {}
        None => {}
    }
}

fn deepseek_provider_core_push_message_item(
    object: &serde_json::Map<String, serde_json::Value>,
    messages: &mut Vec<serde_json::Value>,
    replayed_message_signatures: &BTreeSet<(String, String)>,
) {
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
        messages.push(deepseek_provider_core_chat_message(role, &text));
    }
}

fn deepseek_provider_core_push_tool_call_item(
    object: &serde_json::Map<String, serde_json::Value>,
    messages: &mut Vec<serde_json::Value>,
    replayed_tool_call_ids: &BTreeSet<String>,
    push: fn(&serde_json::Map<String, serde_json::Value>, String, &mut Vec<serde_json::Value>),
) {
    let call_id = deepseek_provider_core_input_tool_call_id(object);
    if replayed_tool_call_ids.contains(&call_id) {
        return;
    }
    push(object, call_id, messages);
}

fn deepseek_provider_core_push_tool_output_item(
    object: &serde_json::Map<String, serde_json::Value>,
    messages: &mut Vec<serde_json::Value>,
    replayed_tool_output_call_ids: &BTreeSet<String>,
) {
    let call_id = deepseek_provider_core_input_tool_output_call_id(object);
    if replayed_tool_output_call_ids.contains(&call_id) {
        return;
    }
    messages.push(deepseek_provider_core_chat_tool_message(
        &call_id,
        &deepseek_provider_core_input_tool_output_text(object),
    ));
}

fn deepseek_provider_core_push_mcp_call_item(
    object: &serde_json::Map<String, serde_json::Value>,
    messages: &mut Vec<serde_json::Value>,
    replayed_tool_call_ids: &BTreeSet<String>,
    replayed_tool_output_call_ids: &BTreeSet<String>,
) {
    let call_id = deepseek_provider_core_input_tool_call_id(object);
    if !replayed_tool_call_ids.contains(&call_id) {
        deepseek_provider_core_push_chat_tool_call_message(object, call_id.clone(), messages);
    }
    if deepseek_provider_core_mcp_call_has_result(object)
        && !replayed_tool_output_call_ids.contains(&call_id)
    {
        messages.push(deepseek_provider_core_chat_tool_message(
            &call_id,
            &deepseek_provider_core_input_tool_output_text(object),
        ));
    }
}

#[cfg(feature = "mojo")]
fn deepseek_provider_core_mojo_value(
    input: prodex_mojo_core::rich::DeepSeekKernelInput<'_>,
) -> serde_json::Value {
    let body = prodex_mojo_core::rich::deepseek_kernel(input)
        .unwrap_or_else(|error| panic!("DeepSeek Mojo kernel failed: {error:?}"));
    serde_json::from_slice(&body)
        .unwrap_or_else(|error| panic!("DeepSeek Mojo kernel returned invalid JSON: {error}"))
}
