//! Gemini built-in request tool mapping.

use serde_json::{Value, json};

pub(crate) fn gemini_builtin_tools_from_request(tools: &[Value]) -> Vec<Value> {
    let mut translated = Vec::new();
    if let Some(computer_use) = gemini_computer_use_tool(tools) {
        translated.push(gemini_builtin_tool_value(1, Some(computer_use)));
    }
    if tools.iter().any(gemini_is_code_execution_tool) {
        translated.push(gemini_builtin_tool_value(2, None));
    }
    if tools.iter().any(gemini_is_web_search_tool) {
        translated.push(gemini_builtin_tool_value(3, None));
    }
    if tools.iter().any(gemini_is_url_context_tool) {
        translated.push(gemini_builtin_tool_value(4, None));
    }
    translated
}

#[cfg(feature = "mojo")]
fn gemini_builtin_tool_value(kind: i64, value: Option<Value>) -> Value {
    let value =
        value.map(|value| serde_json::to_vec(&value).expect("Gemini built-in tool serializes"));
    crate::translators::gemini::request_contents::gemini_request_content_mojo_value(
        prodex_mojo_core::provider_constraints::GeminiRequestContentOperation::BuiltinTool,
        value.as_deref(),
        None,
        None,
        None,
        kind,
    )
}

#[cfg(not(feature = "mojo"))]
fn gemini_builtin_tool_value(kind: i64, value: Option<Value>) -> Value {
    match kind {
        1 => json!({ "computerUse": value.unwrap_or_else(|| json!({})) }),
        2 => json!({ "codeExecution": {} }),
        3 => json!({ "googleSearch": {} }),
        4 => json!({ "urlContext": {} }),
        _ => unreachable!("unknown Gemini built-in tool kind"),
    }
}

pub(crate) fn gemini_is_supported_builtin_tool(tool: &Value) -> bool {
    gemini_is_computer_use_tool(tool)
        || gemini_is_code_execution_tool(tool)
        || gemini_is_web_search_tool(tool)
        || gemini_is_url_context_tool(tool)
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
