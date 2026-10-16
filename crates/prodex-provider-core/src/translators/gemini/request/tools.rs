//! Gemini request tool declarations and tool-choice helpers.

#[path = "tools/builtin.rs"]
mod builtin;

pub(crate) use self::builtin::gemini_builtin_tools_from_request;
use serde_json::{Value, json};

use super::schema::sanitize_function_schema;

pub(crate) fn gemini_validate_openai_tools(value: &Value) -> Result<(), String> {
    gemini_validate_openai_tools_with_kind(value, 0)
}

pub(crate) fn gemini_validate_openai_tools_with_nested_function_paths(
    value: &Value,
) -> Result<(), String> {
    gemini_validate_openai_tools_with_kind(value, 1)
}

fn gemini_validate_openai_tools_with_kind(value: &Value, kind: i64) -> Result<(), String> {
    let Some(tools) = value.as_array() else {
        return gemini_validate_tool_value_with_mojo(value, None, kind);
    };
    for (index, tool) in tools.iter().enumerate() {
        gemini_validate_tool_value_with_mojo(tool, Some(index), kind)?;
    }
    Ok(())
}

fn gemini_validate_tool_value_with_mojo(
    tool: &Value,
    index: Option<usize>,
    kind: i64,
) -> Result<(), String> {
    let request = match index {
        Some(_) => json!({"tools": [tool]}),
        None => json!({"tools": tool}),
    };
    let body = serde_json::to_vec(&request)
        .map_err(|error| format!("failed to serialize Gemini tool validation: {error}"))?;
    let plan = crate::gemini_bridge::gemini_bridge_validate_translator_with_kind(&body, kind)?;
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
    gemini_validate_openai_tools_with_kind(&Value::Array(translated), kind).map_err(|reason| {
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
    let source = prodex_mojo_core::provider_constraints::gemini_function_tool_source(
        tool.get("function").is_some(),
        tool.get("type").and_then(Value::as_str),
    )
    .expect("compiled Mojo Gemini function-tool source returned invalid output");
    let function = match source {
        prodex_mojo_core::provider_constraints::GeminiFunctionToolSource::None => return None,
        prodex_mojo_core::provider_constraints::GeminiFunctionToolSource::Nested => {
            tool.get("function")?
        }
        prodex_mojo_core::provider_constraints::GeminiFunctionToolSource::Flat => tool,
    };
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
