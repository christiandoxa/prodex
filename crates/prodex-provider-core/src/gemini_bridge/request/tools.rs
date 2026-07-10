//! Gemini request tool-shape bridge helpers.

use crate::translators::{
    gemini_builtin_tools_from_request, gemini_function_declaration_from_openai_tool,
    gemini_request_body_without_tool, gemini_sanitize_function_schema,
    gemini_tool_config_from_request,
};

use crate::gemini_bridge::gemini_provider_core_apply_gemini3_tool_declaration_overrides;

pub fn gemini_provider_core_sanitize_function_schema(
    schema: &serde_json::Value,
) -> serde_json::Value {
    gemini_sanitize_function_schema(schema)
}

pub fn gemini_provider_core_tool_config_from_request(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    gemini_tool_config_from_request(value)
}

pub fn gemini_provider_core_function_declaration_from_openai_tool(
    tool: &serde_json::Value,
) -> Option<serde_json::Value> {
    gemini_function_declaration_from_openai_tool(tool)
}

pub fn gemini_provider_core_function_tools_from_chat(
    chat: &serde_json::Value,
    model: &str,
    mut filter_declarations: impl FnMut(&mut Vec<serde_json::Value>),
) -> Option<serde_json::Value> {
    let mut declarations = chat
        .get("tools")?
        .as_array()?
        .iter()
        .filter_map(gemini_provider_core_function_declaration_from_openai_tool)
        .collect::<Vec<_>>();
    gemini_provider_core_apply_gemini3_tool_declaration_overrides(model, &mut declarations);
    filter_declarations(&mut declarations);
    (!declarations.is_empty()).then(|| {
        serde_json::json!([{
            "functionDeclarations": declarations,
        }])
    })
}

pub fn gemini_provider_core_builtin_tools_from_request(
    tools: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    gemini_builtin_tools_from_request(tools)
}

pub fn gemini_provider_core_tools_from_requests(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
    filter_declarations: impl FnMut(&mut Vec<serde_json::Value>),
) -> Option<serde_json::Value> {
    let mut tools = original
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .map(|tools| gemini_provider_core_builtin_tools_from_request(tools))
        .unwrap_or_default();
    if let Some(serde_json::Value::Array(function_tools)) =
        gemini_provider_core_function_tools_from_chat(chat, model, filter_declarations)
    {
        tools.extend(function_tools);
    }
    (!tools.is_empty()).then_some(serde_json::Value::Array(tools))
}

pub fn gemini_provider_core_request_body_without_tool(
    body: &[u8],
    tool_name: &str,
) -> Option<Vec<u8>> {
    gemini_request_body_without_tool(body, tool_name)
}

pub fn gemini_provider_core_unsupported_tool_fallback_body(
    body: &[u8],
) -> Option<(&'static str, Vec<u8>)> {
    ["computerUse", "codeExecution", "urlContext", "googleSearch"]
        .into_iter()
        .find_map(|tool_name| {
            gemini_request_body_without_tool(body, tool_name).map(|body| (tool_name, body))
        })
}
