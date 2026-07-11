//! DeepSeek tool-choice validation.

use std::collections::BTreeSet;

use crate::deepseek_provider_core_json_string;

use super::{
    deepseek_provider_core_function_tool_name, deepseek_provider_core_validate_function_name,
};

pub fn deepseek_provider_core_validate_tool_choice_name(
    tool_choice: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    if let Some(name) = tool_choice
        .get("function")
        .and_then(|function| function.get("name"))
        .and_then(serde_json::Value::as_str)
    {
        deepseek_provider_core_validate_function_name(name, provider_label)?;
    }
    Ok(())
}

pub fn deepseek_provider_core_validate_tool_choice_shape(
    value: &serde_json::Value,
    thinking_enabled: bool,
    provider_label: &str,
) -> Result<(), String> {
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
        return Err(format!(
            "{provider_label} tool_choice string `{choice}` is not supported"
        ));
    }
    let Some(object) = choice.as_object() else {
        return Err(format!(
            "{provider_label} tool_choice must be a string or object"
        ));
    };
    let choice_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if choice_type == "function" || choice_type.starts_with("mcp") {
        if deepseek_provider_core_json_string(object, &["name"])
            .or_else(|| {
                object
                    .get("function")
                    .and_then(serde_json::Value::as_object)
                    .and_then(|function| deepseek_provider_core_json_string(function, &["name"]))
            })
            .filter(|name| !name.trim().is_empty())
            .is_none()
        {
            return Err(format!(
                "{provider_label} named tool_choice requires a function name"
            ));
        }
        return Ok(());
    }
    Err(format!(
        "{provider_label} tool_choice type `{choice_type}` is not supported"
    ))
}

pub fn deepseek_provider_core_validate_tool_choice_target(
    tool_choice: &serde_json::Value,
    tool_names: &BTreeSet<String>,
    provider_label: &str,
) -> Result<(), String> {
    let Some(name) = deepseek_provider_core_function_tool_name(tool_choice) else {
        return Ok(());
    };
    if !tool_names.contains(&name) {
        return Err(format!(
            "{provider_label} named tool_choice `{name}` does not match any translated function tool"
        ));
    }
    Ok(())
}
