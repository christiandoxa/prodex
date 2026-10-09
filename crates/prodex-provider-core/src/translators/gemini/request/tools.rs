//! Gemini request tool declarations and tool-choice helpers.

#[path = "tools/builtin.rs"]
mod builtin;

pub(crate) use self::builtin::gemini_builtin_tools_from_request;
use serde_json::{Value, json};

use super::schema::sanitize_function_schema;

pub(crate) fn gemini_validate_openai_tools(value: &Value) -> Result<(), String> {
    let Some(tools) = value.as_array() else {
        return gemini_validate_tool_value_with_mojo(value, None);
    };
    for (index, tool) in tools.iter().enumerate() {
        gemini_validate_tool_value_with_mojo(tool, Some(index))?;
    }
    Ok(())
}

fn gemini_validate_tool_value_with_mojo(tool: &Value, index: Option<usize>) -> Result<(), String> {
    let request = match index {
        Some(_) => json!({"tools": [tool]}),
        None => json!({"tools": tool}),
    };
    let body = serde_json::to_vec(&request)
        .map_err(|error| format!("failed to serialize Gemini tool validation: {error}"))?;
    let plan = crate::gemini_bridge::gemini_bridge_validate_translator(&body)?;
    if let Some(reason) = plan.reason {
        return Err(gemini_relabel_tool_validation_reason(&reason, index));
    }
    if plan.tag != 16 {
        return Ok(());
    }

    let Some(index) = index else {
        return Err(
            "invalid_tool_declaration: Gemini request field `tools` is not a supported tool declaration"
                .to_string(),
        );
    };
    let translated = crate::chat_tools_bridge::provider_core_chat_tools_from_responses_request(
        &request,
    )
    .ok_or_else(|| {
        format!(
            "invalid_tool_declaration: Gemini request field `tools[{index}]` is not a supported tool declaration"
        )
    })?;
    gemini_validate_openai_tools(&Value::Array(translated)).map_err(|reason| {
        format!(
            "invalid_tool_declaration: Gemini request field `tools[{index}]` translates to an invalid declaration: {reason}"
        )
    })
}

fn gemini_relabel_tool_validation_reason(reason: &str, index: Option<usize>) -> String {
    let Some(index) = index else {
        return reason.to_string();
    };
    reason.replace("tools[0]", &format!("tools[{index}]"))
}

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
        crate::translators::gemini::request_contents::gemini_request_content_mojo_value_or_panic(
            prodex_mojo_core::provider_constraints::GeminiRequestContentOperation::ToolDeclaration,
            Some(&name),
            description.as_deref(),
            Some(&parameters),
            None,
            0,
        ),
    )
}

pub(crate) fn gemini_tool_config_from_request(value: &Value) -> Result<Option<Value>, String> {
    crate::gemini_provider_core_tool_config_from_request(value)
}
