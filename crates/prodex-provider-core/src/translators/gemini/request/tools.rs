//! Gemini request tool declarations and tool-choice helpers.

#[path = "tools/builtin.rs"]
mod builtin;

pub(crate) use self::builtin::{
    gemini_builtin_tools_from_request, gemini_is_supported_builtin_tool,
};
use serde_json::{Value, json};

use super::schema::{sanitize_function_schema, sanitize_schema};

pub(crate) fn gemini_validate_openai_tools(value: &Value) -> Result<(), String> {
    let Some(tools) = value.as_array() else {
        return Err(
            "invalid_tool_declaration: Gemini request field `tools` must be an array".to_string(),
        );
    };
    for (index, tool) in tools.iter().enumerate() {
        gemini_validate_openai_tool(tool, index)?;
    }
    Ok(())
}

fn gemini_validate_openai_tool(tool: &Value, index: usize) -> Result<(), String> {
    let Some(object) = tool.as_object() else {
        return Err(format!(
            "invalid_tool_declaration: Gemini request field `tools[{index}]` must be an object"
        ));
    };
    let is_function = object
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|tool_type| tool_type == "function")
        || object.contains_key("function");
    if is_function {
        return gemini_validate_function_tool(object, index);
    }
    if gemini_is_supported_builtin_tool(tool) {
        return Ok(());
    }
    let translated = crate::chat_tools_bridge::provider_core_chat_tools_from_responses_request(
        &json!({"tools": [tool]}),
    )
    .ok_or_else(|| {
        let field = if object.contains_key("type") {
            format!("tools[{index}].type")
        } else {
            format!("tools[{index}]")
        };
        format!(
            "invalid_tool_declaration: Gemini request field `{field}` is not a supported tool declaration"
        )
    })?;
    gemini_validate_openai_tools(&Value::Array(translated)).map_err(|reason| {
        format!(
            "invalid_tool_declaration: Gemini request field `tools[{index}]` translates to an invalid declaration: {reason}"
        )
    })
}

fn gemini_validate_function_tool(
    object: &serde_json::Map<String, Value>,
    index: usize,
) -> Result<(), String> {
    let (function, field) = match object.get("function") {
        Some(function) => (
            function.as_object().ok_or_else(|| {
                format!(
                    "invalid_tool_declaration: Gemini request field `tools[{index}].function` must be an object"
                )
            })?,
            format!("tools[{index}].function"),
        ),
        None => (object, format!("tools[{index}]")),
    };
    if function
        .get("name")
        .and_then(Value::as_str)
        .is_none_or(|name| name.trim().is_empty())
    {
        return Err(format!(
            "invalid_tool_declaration: Gemini request field `{field}.name` must be a non-empty string"
        ));
    }
    let Some(parameters) = function.get("parameters") else {
        return Err(format!(
            "invalid_tool_declaration: Gemini request field `{field}.parameters` is required"
        ));
    };
    if !parameters.is_object() {
        return Err(format!(
            "invalid_tool_declaration: Gemini request field `{field}.parameters` must be an object"
        ));
    }
    if let Some(description) = function.get("description").filter(|value| !value.is_null())
        && !description.is_string()
    {
        return Err(format!(
            "invalid_tool_declaration: Gemini request field `{field}.description` must be a string"
        ));
    }
    Ok(())
}

#[cfg(feature = "mojo")]
pub(crate) fn gemini_tool_from_openai_tool(tool: &Value, index: usize) -> Result<Value, String> {
    let Some(function) = tool.get("function") else {
        return Err(format!(
            "invalid_tool_declaration: Gemini request field `tools[{index}].function` must be an object"
        ));
    };
    let Some(name) = function.get("name").and_then(Value::as_str) else {
        return Err(format!(
            "invalid_tool_declaration: Gemini request field `tools[{index}].function.name` must be a non-empty string"
        ));
    };
    if name.trim().is_empty() {
        return Err(format!(
            "invalid_tool_declaration: Gemini request field `tools[{index}].function.name` must be a non-empty string"
        ));
    }
    let description = function
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let parameters = function
        .get("parameters")
        .map(sanitize_schema)
        .unwrap_or_else(|| json!({"type":"object","properties":{}}));
    let name = serde_json::to_vec(name).expect("Gemini tool name serializes");
    let description = serde_json::to_vec(description).expect("Gemini tool description serializes");
    let parameters = serde_json::to_vec(&parameters).expect("Gemini tool parameters serialize");
    Ok(
        crate::translators::gemini::request_contents::gemini_request_content_mojo_value(
            prodex_mojo_core::provider_constraints::GeminiRequestContentOperation::ToolDeclaration,
            Some(&name),
            Some(&description),
            Some(&parameters),
            None,
            0,
        ),
    )
}

#[cfg(not(feature = "mojo"))]
pub(crate) fn gemini_tool_from_openai_tool(tool: &Value, index: usize) -> Result<Value, String> {
    let Some(function) = tool.get("function") else {
        return Err(format!(
            "invalid_tool_declaration: Gemini request field `tools[{index}].function` must be an object"
        ));
    };
    let Some(name) = function.get("name").and_then(Value::as_str) else {
        return Err(format!(
            "invalid_tool_declaration: Gemini request field `tools[{index}].function.name` must be a non-empty string"
        ));
    };
    if name.trim().is_empty() {
        return Err(format!(
            "invalid_tool_declaration: Gemini request field `tools[{index}].function.name` must be a non-empty string"
        ));
    }
    let description = function
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let parameters = function
        .get("parameters")
        .map(sanitize_schema)
        .unwrap_or_else(|| json!({"type":"object","properties":{}}));
    Ok(json!({
        "name": name,
        "description": description,
        "parameters": parameters,
    }))
}

#[cfg(feature = "mojo")]
pub(crate) fn gemini_function_declaration_from_openai_tool(tool: &Value) -> Option<Value> {
    let function = tool.get("function")?;
    let name = function.get("name").and_then(Value::as_str)?;
    let default_parameters = json!({"type": "object"});
    let parameters = function.get("parameters").unwrap_or(&default_parameters);
    let parameters = sanitize_function_schema(parameters);
    let name = serde_json::to_vec(name).expect("Gemini tool name serializes");
    let parameters = serde_json::to_vec(&parameters).expect("Gemini tool parameters serialize");
    let description = function
        .get("description")
        .and_then(Value::as_str)
        .map(|description| {
            serde_json::to_vec(description).expect("Gemini tool description serializes")
        });
    Some(
        crate::translators::gemini::request_contents::gemini_request_content_mojo_value(
            prodex_mojo_core::provider_constraints::GeminiRequestContentOperation::ToolDeclaration,
            Some(&name),
            description.as_deref(),
            Some(&parameters),
            None,
            0,
        ),
    )
}

#[cfg(not(feature = "mojo"))]
pub(crate) fn gemini_function_declaration_from_openai_tool(tool: &Value) -> Option<Value> {
    let function = tool.get("function")?;
    let name = function.get("name").and_then(Value::as_str)?;
    let default_parameters = json!({"type": "object"});
    let parameters = function.get("parameters").unwrap_or(&default_parameters);
    let mut declaration = json!({
        "name": name,
        "parameters": sanitize_function_schema(parameters),
    });
    if let Some(description) = function.get("description").and_then(Value::as_str) {
        declaration["description"] = Value::String(description.to_string());
    }
    Some(declaration)
}

#[cfg(feature = "mojo")]
pub(crate) fn gemini_tool_config_from_request(value: &Value) -> Option<Value> {
    let tool_choice = value.get("tool_choice")?;
    let (mode, name) = if tool_choice.as_str() == Some("auto") {
        return None;
    } else if tool_choice.as_str() == Some("none") {
        ("NONE", None)
    } else if tool_choice.as_str() == Some("required") {
        ("ANY", None)
    } else {
        let name = tool_choice
            .get("function")
            .and_then(|function| function.get("name"))
            .and_then(Value::as_str)
            .or_else(|| tool_choice.get("name").and_then(Value::as_str))?;
        ("ANY", Some(name))
    };
    let mode = serde_json::to_vec(mode).expect("Gemini tool mode serializes");
    let name = name.map(|name| serde_json::to_vec(name).expect("Gemini tool name serializes"));
    Some(
        crate::translators::gemini::request_contents::gemini_request_content_mojo_value(
            prodex_mojo_core::provider_constraints::GeminiRequestContentOperation::ToolConfig,
            Some(&mode),
            name.as_deref(),
            None,
            None,
            0,
        ),
    )
}

#[cfg(not(feature = "mojo"))]
pub(crate) fn gemini_tool_config_from_request(value: &Value) -> Option<Value> {
    let tool_choice = value.get("tool_choice")?;
    if tool_choice.as_str() == Some("auto") {
        return None;
    }
    if tool_choice.as_str() == Some("none") {
        return Some(json!({
            "functionCallingConfig": {
                "mode": "NONE",
            }
        }));
    }
    if tool_choice.as_str() == Some("required") {
        return Some(json!({
            "functionCallingConfig": {
                "mode": "ANY",
            }
        }));
    }
    let name = tool_choice
        .get("function")
        .and_then(|function| function.get("name"))
        .and_then(Value::as_str)
        .or_else(|| tool_choice.get("name").and_then(Value::as_str))?;
    Some(json!({
        "functionCallingConfig": {
            "mode": "ANY",
            "allowedFunctionNames": [name],
        }
    }))
}
