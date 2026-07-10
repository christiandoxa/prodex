//! DeepSeek tool object shape validators.

use super::super::deepseek_provider_core_json_string;

mod mcp;

pub(super) use self::mcp::{
    deepseek_provider_core_validate_mcp_function_tool_shape,
    deepseek_provider_core_validate_mcp_toolset_shape,
};

pub(super) fn deepseek_provider_core_validate_optional_string(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    label: &str,
    provider_label: &str,
) -> Result<(), String> {
    if let Some(value) = object.get(key)
        && !value.is_string()
    {
        return Err(format!("{provider_label} {label} must be a string"));
    }
    Ok(())
}

pub(super) fn deepseek_provider_core_validate_optional_bool(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    label: &str,
    provider_label: &str,
) -> Result<(), String> {
    if let Some(value) = object.get(key)
        && !value.is_boolean()
    {
        return Err(format!("{provider_label} {label} must be a boolean"));
    }
    Ok(())
}

pub(super) fn deepseek_provider_core_validate_custom_tool_format_shape(
    object: &serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    let Some(format) = object.get("format") else {
        return Ok(());
    };
    let Some(format) = format.as_object() else {
        return Err(format!(
            "{provider_label} custom tool format must be an object"
        ));
    };
    if let Some(format_type) = format.get("type")
        && !format_type.is_string()
    {
        return Err(format!(
            "{provider_label} custom tool format.type must be a string"
        ));
    }
    Ok(())
}

pub(super) fn deepseek_provider_core_validate_namespace_tool_shape(
    object: &serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    let Some(namespace) = deepseek_provider_core_json_string(object, &["name"])
        .filter(|name| !name.trim().is_empty())
    else {
        return Err(format!("{provider_label} namespace tools require a name"));
    };
    let Some(tools) = object.get("tools").and_then(serde_json::Value::as_array) else {
        return Err(format!(
            "{provider_label} namespace tool `{namespace}` requires a tools array"
        ));
    };
    if tools.is_empty() {
        return Err(format!(
            "{provider_label} namespace tool `{namespace}` requires at least one tool"
        ));
    }
    for tool in tools {
        let Some(tool) = tool.as_object() else {
            return Err(format!(
                "{provider_label} namespace tool `{namespace}` entries must be objects"
            ));
        };
        deepseek_provider_core_validate_optional_string(
            tool,
            "description",
            "namespace function description",
            provider_label,
        )?;
        deepseek_provider_core_validate_optional_bool(
            tool,
            "strict",
            "namespace function strict",
            provider_label,
        )?;
        if tool.get("type").and_then(serde_json::Value::as_str) != Some("function") {
            return Err(format!(
                "{provider_label} namespace tool `{namespace}` entries must be function tools"
            ));
        }
        if deepseek_provider_core_json_string(tool, &["name"])
            .filter(|name| !name.trim().is_empty())
            .is_none()
        {
            return Err(format!(
                "{provider_label} namespace tool `{namespace}` function entries require a name"
            ));
        }
    }
    Ok(())
}
