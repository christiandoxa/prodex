//! DeepSeek top-level tools-array shape validation.

#[cfg(feature = "mojo")]
use crate::deepseek_bridge::request_policy::{detail, plan_value};
#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::DeepSeekRequestPolicyOperation;

#[cfg(not(feature = "mojo"))]
use super::function_tools::deepseek_provider_core_tool_name_from_tool_object;
#[cfg(not(feature = "mojo"))]
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
    #[cfg(feature = "mojo")]
    {
        let (source, plan) = plan_value(
            value,
            DeepSeekRequestPolicyOperation::ToolsShape,
            gemini_compat,
        );
        let detail = || detail(&source, plan).unwrap_or_default();
        return match plan.tag {
            0 => Ok(()),
            1 => Err(format!("{provider_label} tools must be an array")),
            2 => Err(format!("{provider_label} tools entries must be objects")),
            3 => Err(format!(
                "{provider_label} tool type \x60{}\x60 is not supported",
                detail()
            )),
            4 => Err(format!(
                "{provider_label} tool description must be a string"
            )),
            5 => Err(format!(
                "{provider_label} function description must be a string"
            )),
            6 => Err(format!(
                "{provider_label} function strict must be a boolean"
            )),
            7 => Err(format!("{provider_label} tool strict must be a boolean")),
            8 => Err(format!(
                "{provider_label} {} tools require a name",
                detail()
            )),
            9 => Err(format!(
                "{provider_label} custom tools cannot preserve strict=true through the function wrapper"
            )),
            10 => Err(format!(
                "{provider_label} custom tool format must be an object"
            )),
            11 => Err(format!(
                "{provider_label} custom tool format.type must be a string"
            )),
            12 => Err(format!("{provider_label} namespace tools require a name")),
            13 => Err(format!(
                "{provider_label} namespace tool \x60{}\x60 requires a tools array",
                detail()
            )),
            14 => Err(format!(
                "{provider_label} namespace tool \x60{}\x60 requires at least one tool",
                detail()
            )),
            15 => Err(format!(
                "{provider_label} namespace tool \x60{}\x60 entries must be objects",
                detail()
            )),
            16 => Err(format!(
                "{provider_label} namespace function description must be a string"
            )),
            17 => Err(format!(
                "{provider_label} namespace function strict must be a boolean"
            )),
            18 => Err(format!(
                "{provider_label} namespace tool \x60{}\x60 entries must be function tools",
                detail()
            )),
            19 => Err(format!(
                "{provider_label} namespace tool \x60{}\x60 function entries require a name",
                detail()
            )),
            20 => Err(format!(
                "{provider_label} MCP function tools require a name"
            )),
            21 => Err(format!(
                "{provider_label} MCP function tool \x60{}\x60 requires a schema",
                detail()
            )),
            22 => Err(format!(
                "{provider_label} MCP toolsets require a server name"
            )),
            23 => Err(format!(
                "{provider_label} MCP toolset \x60{}\x60 requires allowed_tools or enabled configs",
                detail()
            )),
            _ => Err(format!(
                "{provider_label} tools shape validation returned an unknown result"
            )),
        };
    }

    #[cfg(not(feature = "mojo"))]
    {
        let Some(tools) = value.get("tools") else {
            return Ok(());
        };
        let Some(tools) = tools.as_array() else {
            return Err(format!("{provider_label} tools must be an array"));
        };
        if tools.iter().any(|tool| !tool.is_object()) {
            return Err(format!("{provider_label} tools entries must be objects"));
        };
        for tool in tools {
            let Some(object) = tool.as_object() else {
                continue;
            };
            let Some(tool_type) = object.get("type").and_then(serde_json::Value::as_str) else {
                continue;
            };
            if deepseek_provider_core_is_supported_tool_type(tool_type, gemini_compat) {
                deepseek_provider_core_validate_supported_tool_shape(
                    object,
                    tool_type,
                    provider_label,
                )?;
                continue;
            }
            return Err(format!(
                "{provider_label} tool type \x60{tool_type}\x60 is not supported"
            ));
        }
        Ok(())
    }
}

#[cfg(not(feature = "mojo"))]
fn deepseek_provider_core_is_supported_tool_type(tool_type: &str, gemini_compat: bool) -> bool {
    matches!(
        tool_type,
        "function"
            | "custom"
            | "namespace"
            | "tool_search"
            | "mcp"
            | "mcp_toolset"
            | "web_search"
            | "web_search_preview"
    ) || tool_type.starts_with("mcp")
        || tool_type.starts_with("web_search_preview_")
        || (gemini_compat && deepseek_provider_core_is_gemini_tool_type(tool_type))
}

#[cfg(not(feature = "mojo"))]
fn deepseek_provider_core_is_gemini_tool_type(tool_type: &str) -> bool {
    matches!(
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
    ) || tool_type.starts_with("web_fetch_preview_")
        || tool_type.starts_with("computer_")
}

#[cfg(not(feature = "mojo"))]
fn deepseek_provider_core_validate_supported_tool_shape(
    object: &serde_json::Map<String, serde_json::Value>,
    tool_type: &str,
    provider_label: &str,
) -> Result<(), String> {
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
    deepseek_provider_core_validate_optional_bool(object, "strict", "tool strict", provider_label)?;
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
    Ok(())
}
