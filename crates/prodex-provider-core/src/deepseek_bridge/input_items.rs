//! Responses input/history conversion helpers for the DeepSeek chat bridge.

use std::collections::BTreeSet;

mod history;
mod validation;

pub use self::history::{
    deepseek_provider_core_chat_role, deepseek_provider_core_first_function_call_output_call_id,
    deepseek_provider_core_history_has_system_message,
    deepseek_provider_core_history_has_tool_call, deepseek_provider_core_message_signatures,
    deepseek_provider_core_tool_call_ids, deepseek_provider_core_tool_output_call_ids,
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
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::SystemMessage,
    );
    input.content = Some(content);
    deepseek_provider_core_mojo_value(input)
}

pub fn deepseek_provider_core_user_message(content: &str) -> serde_json::Value {
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::UserMessage,
    );
    input.content = Some(content);
    deepseek_provider_core_mojo_value(input)
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
    let item_type = object.get("type").and_then(serde_json::Value::as_str);
    let call_id = object
        .get("call_id")
        .or_else(|| object.get("tool_call_id"))
        .or_else(|| object.get("id"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("call_0");
    let is_mcp = item_type == Some("mcp_call");
    let is_call = matches!(
        item_type,
        Some("function_call" | "custom_tool_call" | "local_shell_call" | "mcp_call")
    );
    let is_output = matches!(
        item_type,
        Some(
            "function_call_output"
                | "custom_tool_call_output"
                | "mcp_tool_result"
                | "mcp_call_output"
        )
    );
    if !is_call && !is_output {
        let role = deepseek_provider_core_chat_role(
            object
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("user"),
        );
        let text = deepseek_provider_core_responses_content_text(object.get("content"));
        if text.trim().is_empty() || replayed_message_signatures.contains(&(role.to_string(), text))
        {
            return;
        }
    }

    let emit_call = !is_call || !replayed_tool_call_ids.contains(call_id);
    let emit_output = is_mcp
        && ["output", "content", "result", "error"]
            .iter()
            .any(|key| object.contains_key(*key))
        && !replayed_tool_output_call_ids.contains(call_id);
    if (is_call && !emit_call && !emit_output)
        || (is_output && replayed_tool_output_call_ids.contains(call_id))
    {
        return;
    }

    let source = serde_json::to_string(item).expect("DeepSeek input item serializes");
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::RawBridgeInputItem,
    );
    input.item = Some(&source);
    input.stream = if is_output { true } else { emit_call };
    input.sequence_number = u64::from(emit_output);
    let body = prodex_mojo_core::rich::deepseek_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo DeepSeek input-item shaping failed: {error:?}"));
    let mapped: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!("Mojo DeepSeek input-item shaping returned invalid JSON: {error}")
    });
    messages.extend(mapped);
}

fn deepseek_provider_core_mojo_value(
    input: prodex_mojo_core::rich::DeepSeekKernelInput<'_>,
) -> serde_json::Value {
    let body = prodex_mojo_core::rich::deepseek_kernel(input)
        .unwrap_or_else(|error| panic!("DeepSeek Mojo kernel failed: {error:?}"));
    serde_json::from_slice(&body)
        .unwrap_or_else(|error| panic!("DeepSeek Mojo kernel returned invalid JSON: {error}"))
}
