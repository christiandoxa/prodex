//! DeepSeek Responses input item validation helpers.

use super::super::{
    deepseek_provider_core_json_string, deepseek_provider_core_json_string_at_path,
};

mod content;

pub(super) use self::content::{
    deepseek_provider_core_reject_chat_prefix_marker,
    deepseek_provider_core_validate_supported_message_content,
};

pub(super) fn deepseek_provider_core_validate_input_message_role(
    object: &serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    let Some(role) = object.get("role") else {
        return Ok(());
    };
    let Some(role) = role.as_str() else {
        return Err(format!("{provider_label} message role must be a string"));
    };
    if matches!(role, "user" | "assistant" | "system" | "developer") {
        return Ok(());
    }
    Err(format!(
        "{provider_label} message role `{role}` is not supported by this Responses adapter"
    ))
}

pub(super) fn deepseek_provider_core_validate_input_tool_call_item(
    object: &serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    deepseek_provider_core_require_input_item_call_id(object, provider_label)?;
    if deepseek_provider_core_input_item_tool_name(object).is_none() {
        return Err(format!(
            "{provider_label} input tool call items require a function name"
        ));
    }
    Ok(())
}

pub(super) fn deepseek_provider_core_validate_input_tool_output_item(
    object: &serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    deepseek_provider_core_require_input_item_call_id(object, provider_label)?;
    if ["output", "content", "result", "error"]
        .iter()
        .any(|key| object.contains_key(*key))
    {
        return Ok(());
    }
    Err(format!(
        "{provider_label} input tool output items require output content"
    ))
}

pub(super) fn deepseek_provider_core_validate_input_local_shell_call_item(
    object: &serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    deepseek_provider_core_require_input_item_call_id(object, provider_label)?;
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
        return Err(format!(
            "{provider_label} local_shell_call action.command must be an array of strings"
        ));
    }
    if deepseek_provider_core_json_string(object, &["command"])
        .filter(|command| !command.trim().is_empty())
        .is_some()
    {
        return Ok(());
    }
    Err(format!(
        "{provider_label} local_shell_call requires a command"
    ))
}

pub(super) fn deepseek_provider_core_require_input_item_call_id(
    object: &serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    if deepseek_provider_core_json_string(object, &["call_id", "tool_call_id", "id"])
        .filter(|call_id| !call_id.trim().is_empty())
        .is_some()
    {
        return Ok(());
    }
    Err(format!(
        "{provider_label} input tool items require a call_id"
    ))
}

pub(super) fn deepseek_provider_core_input_item_tool_name(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    deepseek_provider_core_json_string(object, &["name", "tool_name"])
        .or_else(|| deepseek_provider_core_json_string_at_path(object, &["function", "name"]))
        .filter(|name| !name.trim().is_empty())
}
