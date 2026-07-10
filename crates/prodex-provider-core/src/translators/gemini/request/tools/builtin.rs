//! Gemini built-in request tool mapping.

use serde_json::{Value, json};

pub(crate) fn gemini_builtin_tools_from_request(tools: &[Value]) -> Vec<Value> {
    let mut translated = Vec::new();
    if let Some(computer_use) = gemini_computer_use_tool(tools) {
        translated.push(json!({ "computerUse": computer_use }));
    }
    if tools.iter().any(gemini_is_code_execution_tool) {
        translated.push(json!({ "codeExecution": {} }));
    }
    if tools.iter().any(gemini_is_web_search_tool) {
        translated.push(json!({ "googleSearch": {} }));
    }
    if tools.iter().any(gemini_is_url_context_tool) {
        translated.push(json!({ "urlContext": {} }));
    }
    translated
}

fn gemini_computer_use_tool(tools: &[Value]) -> Option<Value> {
    let tool = tools
        .iter()
        .find(|tool| gemini_is_computer_use_tool(tool))?;
    let source = tool
        .get("computerUse")
        .or_else(|| tool.get("computer_use"))
        .unwrap_or(tool);
    let environment = source
        .get("environment")
        .and_then(Value::as_str)
        .filter(|environment| !environment.trim().is_empty())
        .unwrap_or("ENVIRONMENT_BROWSER");
    let mut computer_use = json!({
        "environment": environment,
    });
    if let Some(excluded) = source
        .get("excludedPredefinedFunctions")
        .or_else(|| source.get("excluded_predefined_functions"))
        .filter(|value| !value.is_null())
    {
        computer_use["excludedPredefinedFunctions"] = excluded.clone();
    }
    Some(computer_use)
}

fn gemini_is_computer_use_tool(tool: &Value) -> bool {
    let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or_default();
    matches!(
        tool_type,
        "computer" | "computer_use" | "computerUse" | "computer_use_preview"
    ) || tool_type.starts_with("computer_")
        || tool
            .as_object()
            .is_some_and(|object| object.contains_key("computerUse"))
}

fn gemini_is_code_execution_tool(tool: &Value) -> bool {
    let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or_default();
    matches!(
        tool_type,
        "code_interpreter" | "code_execution" | "codeExecution"
    ) || tool
        .as_object()
        .is_some_and(|object| object.contains_key("codeExecution"))
}

fn gemini_is_web_search_tool(tool: &Value) -> bool {
    let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or_default();
    tool_type == "web_search"
        || tool_type == "web_search_preview"
        || tool_type.starts_with("web_search_preview_")
}

fn gemini_is_url_context_tool(tool: &Value) -> bool {
    let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or_default();
    tool_type == "web_fetch"
        || tool_type == "url_context"
        || tool_type == "urlContext"
        || tool_type == "web_fetch_preview"
        || tool_type.starts_with("web_fetch_preview_")
        || tool
            .as_object()
            .is_some_and(|object| object.contains_key("urlContext"))
}
