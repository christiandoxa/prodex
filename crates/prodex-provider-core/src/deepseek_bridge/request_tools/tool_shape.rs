//! DeepSeek top-level tools-array shape validation.

use super::function_tools::deepseek_provider_core_tool_name_from_tool_object;
use super::shape::{
    deepseek_provider_core_validate_custom_tool_format_shape,
    deepseek_provider_core_validate_mcp_function_tool_shape,
    deepseek_provider_core_validate_mcp_toolset_shape,
    deepseek_provider_core_validate_namespace_tool_shape,
    deepseek_provider_core_validate_optional_bool, deepseek_provider_core_validate_optional_string,
};

pub fn deepseek_provider_core_validate_tools_shape(
    value: &serde_json::Value,
    gemini_compat: bool,
    provider_label: &str,
) -> Result<(), String> {
    let Some(tools) = value.get("tools") else {
        return Ok(());
    };
    let Some(tools) = tools.as_array() else {
        return Err(format!("{provider_label} tools must be an array"));
    };
    if tools.iter().any(|tool| !tool.is_object()) {
        return Err(format!("{provider_label} tools entries must be objects"));
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
            deepseek_provider_core_validate_optional_string(
                object,
                "description",
                "tool description",
                provider_label,
            )?;
            if let Some(function) = object
                .get("function")
                .and_then(serde_json::Value::as_object)
            {
                deepseek_provider_core_validate_optional_string(
                    function,
                    "description",
                    "function description",
                    provider_label,
                )?;
                deepseek_provider_core_validate_optional_bool(
                    function,
                    "strict",
                    "function strict",
                    provider_label,
                )?;
            }
            deepseek_provider_core_validate_optional_bool(
                object,
                "strict",
                "tool strict",
                provider_label,
            )?;
            if matches!(tool_type, "function" | "custom")
                && deepseek_provider_core_tool_name_from_tool_object(object).is_none()
            {
                return Err(format!("{provider_label} {tool_type} tools require a name"));
            }
            if tool_type == "custom"
                && object.get("strict").and_then(serde_json::Value::as_bool) == Some(true)
            {
                return Err(format!(
                    "{provider_label} custom tools cannot preserve strict=true through the function wrapper"
                ));
            }
            if tool_type == "custom" {
                deepseek_provider_core_validate_custom_tool_format_shape(object, provider_label)?;
            }
            if tool_type == "namespace" {
                deepseek_provider_core_validate_namespace_tool_shape(object, provider_label)?;
            }
            if matches!(tool_type, "mcp" | "mcp_toolset") {
                deepseek_provider_core_validate_mcp_toolset_shape(object, provider_label)?;
            }
            if tool_type.starts_with("mcp") && !matches!(tool_type, "mcp" | "mcp_toolset") {
                deepseek_provider_core_validate_mcp_function_tool_shape(object, provider_label)?;
            }
            continue;
        }
        return Err(format!(
            "{provider_label} tool type `{tool_type}` is not supported"
        ));
    }
    Ok(())
}
