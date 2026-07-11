//! DeepSeek function-tool name and parameter validation.

use crate::deepseek_provider_core_json_string;

pub fn deepseek_provider_core_function_tool_name(tool: &serde_json::Value) -> Option<String> {
    tool.get("function")
        .and_then(|function| function.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

pub fn deepseek_provider_core_tool_name_from_tool_object(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    deepseek_provider_core_json_string(object, &["name"])
        .or_else(|| {
            object
                .get("function")
                .and_then(serde_json::Value::as_object)
                .and_then(|function| deepseek_provider_core_json_string(function, &["name"]))
        })
        .filter(|name| !name.trim().is_empty())
}

pub fn deepseek_provider_core_is_generic_function_tool(tool: &serde_json::Value) -> bool {
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

pub fn deepseek_provider_core_validate_function_name(
    name: &str,
    provider_label: &str,
) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(format!(
            "{provider_label} function tool names must use only letters, numbers, underscores, or dashes and be at most 64 bytes"
        ));
    }
    Ok(())
}

pub fn deepseek_provider_core_validate_function_parameters(
    tool: &serde_json::Value,
    name: &str,
    provider_label: &str,
) -> Result<(), String> {
    if let Some(parameters) = tool
        .get("function")
        .and_then(|function| function.get("parameters"))
        && !parameters.is_object()
    {
        return Err(format!(
            "{provider_label} function tool `{name}` parameters must be an object"
        ));
    }
    Ok(())
}
