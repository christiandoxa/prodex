//! Gemini request tool-shape bridge helpers.

use crate::translators::{
    gemini_builtin_tools_from_request, gemini_function_declaration_from_openai_tool,
    gemini_request_body_without_tool, gemini_sanitize_function_schema,
    gemini_tool_config_from_request, gemini_validate_openai_tools,
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

fn gemini_provider_core_function_declaration_from_openai_tool(
    tool: &serde_json::Value,
) -> Option<serde_json::Value> {
    if tool.get("function").is_some() {
        return gemini_function_declaration_from_openai_tool(tool);
    }
    if tool.get("type").and_then(serde_json::Value::as_str) != Some("function") {
        return None;
    }
    let object = tool.as_object()?;
    let mut function = serde_json::Map::new();
    for field in ["name", "description", "parameters"] {
        if let Some(value) = object.get(field) {
            function.insert(field.to_string(), value.clone());
        }
    }
    let mut normalized = object.clone();
    normalized.insert("function".to_string(), serde_json::Value::Object(function));
    gemini_function_declaration_from_openai_tool(&serde_json::Value::Object(normalized))
}

pub fn gemini_provider_core_function_tools_from_chat(
    chat: &serde_json::Value,
    model: &str,
    mut filter_declarations: impl FnMut(&mut Vec<serde_json::Value>),
) -> Result<Option<serde_json::Value>, String> {
    gemini_provider_core_validate_request_tools(chat)?;
    let Some(tools) = chat.get("tools").and_then(serde_json::Value::as_array) else {
        return Ok(None);
    };
    let mut declarations = Vec::new();
    for (index, tool) in tools.iter().enumerate() {
        if !gemini_builtin_tools_from_request(std::slice::from_ref(tool)).is_empty() {
            continue;
        }
        let declaration = gemini_provider_core_function_declaration_from_openai_tool(tool)
            .ok_or_else(|| {
                format!(
                    "invalid_tool_declaration: Gemini request field `tools[{index}]` could not be translated"
                )
            })?;
        declarations.push(declaration);
    }
    gemini_provider_core_apply_gemini3_tool_declaration_overrides(model, &mut declarations);
    filter_declarations(&mut declarations);
    Ok((!declarations.is_empty()).then(|| {
        serde_json::json!([{
            "functionDeclarations": declarations,
        }])
    }))
}

pub fn gemini_provider_core_function_tools_from_chat_checked(
    chat: &serde_json::Value,
    model: &str,
    filter_declarations: impl FnMut(&mut Vec<serde_json::Value>),
) -> Result<Option<serde_json::Value>, String> {
    gemini_provider_core_function_tools_from_chat(chat, model, filter_declarations)
}

fn gemini_provider_core_builtin_tools_from_request(
    tools: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    gemini_builtin_tools_from_request(tools)
}

pub fn gemini_provider_core_tools_from_requests(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
    filter_declarations: impl FnMut(&mut Vec<serde_json::Value>),
) -> Result<Option<serde_json::Value>, String> {
    gemini_provider_core_validate_request_tools(original)?;
    gemini_provider_core_validate_request_tools(chat)?;
    let mut tools = original
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .map(|tools| gemini_provider_core_builtin_tools_from_request(tools))
        .unwrap_or_default();
    if let Some(serde_json::Value::Array(function_tools)) =
        gemini_provider_core_function_tools_from_chat(chat, model, filter_declarations)?
    {
        tools.extend(function_tools);
    }
    Ok((!tools.is_empty()).then_some(serde_json::Value::Array(tools)))
}

pub fn gemini_provider_core_tools_from_requests_checked(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
    filter_declarations: impl FnMut(&mut Vec<serde_json::Value>),
) -> Result<Option<serde_json::Value>, String> {
    gemini_provider_core_tools_from_requests(original, chat, model, filter_declarations)
}

pub fn gemini_provider_core_validate_request_tools(
    value: &serde_json::Value,
) -> Result<(), String> {
    match value.get("tools") {
        Some(tools) => gemini_validate_openai_tools(tools),
        None => Ok(()),
    }
}

pub fn gemini_provider_core_request_body_without_tool(
    body: &[u8],
    tool_name: &str,
) -> Option<Vec<u8>> {
    gemini_request_body_without_tool(body, tool_name)
}

pub fn gemini_provider_core_unsupported_tool_fallback_body(
    body: &[u8],
    error_body: &[u8],
) -> Option<(&'static str, Vec<u8>)> {
    ["computerUse", "codeExecution", "urlContext", "googleSearch"]
        .into_iter()
        .filter(|tool_name| crate::provider_error_rejects_request_member(error_body, tool_name))
        .find_map(|tool_name| {
            gemini_request_body_without_tool(body, tool_name).map(|body| (tool_name, body))
        })
}
