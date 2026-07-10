//! Gemini request tool declarations and tool-choice helpers.

#[path = "tools/builtin.rs"]
mod builtin;

pub(crate) use self::builtin::gemini_builtin_tools_from_request;
use serde_json::{Value, json};

use super::schema::{sanitize_function_schema, sanitize_schema};

pub(crate) fn gemini_tool_from_openai_tool(tool: &Value) -> Option<Value> {
    let function = tool.get("function")?;
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("tool_call");
    let description = function
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let parameters = function
        .get("parameters")
        .map(sanitize_schema)
        .unwrap_or_else(|| json!({"type":"object","properties":{}}));
    Some(json!({
        "name": name,
        "description": description,
        "parameters": parameters,
    }))
}

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
