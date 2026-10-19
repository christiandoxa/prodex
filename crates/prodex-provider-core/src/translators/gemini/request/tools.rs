//! Gemini request tool declarations and tool-choice helpers.

#[path = "tools/builtin.rs"]
mod builtin;

pub(crate) use self::builtin::{gemini_builtin_tool_indices, gemini_builtin_tools_from_request};
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
    // One compiled full-array scan preserves the original invalid-tool index.
    // For an unsupported Responses tool, validate its Mojo-translated
    // declarations and mark only that original index as processed. Mojo must
    // still check all later tools, never silently accept their bad shapes.
    let body = serde_json::to_vec(&json!({"tools": value}))
        .map_err(|error| format!("failed to serialize Gemini tool validation: {error}"))?;
    let mut verified = value.as_array().map(|items| vec![b'0'; items.len()]);
    loop {
        let mask = verified.as_ref().map(|bits| {
            serde_json::to_vec(std::str::from_utf8(bits).expect("ASCII verification mask"))
                .expect("Gemini tool verification mask serializes")
        });
        let plan = crate::gemini_bridge::gemini_bridge_validate_tool_array_with_kind(
            &body,
            kind,
            mask.as_deref(),
        )?;
        if let Some(reason) = plan.reason {
            return Err(reason);
        }
        if plan.tag == 0 {
            return Ok(());
        }
        if plan.tag != 16 {
            return Err(format!(
                "Mojo Gemini tool validation returned unexpected tag {}",
                plan.tag
            ));
        }
        let Some(tools) = value.as_array() else {
            return Err(
                "invalid_tool_declaration: Gemini request field `tools` is not a supported tool declaration"
                    .to_string(),
            );
        };
        let index = usize::try_from(plan.index).map_err(|_| {
            "Mojo Gemini tool validation returned an invalid array index".to_string()
        })?;
        let tool = tools.get(index).ok_or_else(|| {
            "Mojo Gemini tool validation returned an out-of-range index".to_string()
        })?;
        let verified = verified.as_mut().ok_or_else(|| {
            "Mojo Gemini tool validation returned a missing verification mask".to_string()
        })?;
        if verified[index] != b'0' {
            return Err("Mojo Gemini tool validation returned a repeated bridge index".to_string());
        }
        let request = json!({"tools": [tool]});
        let converted = crate::chat_tools_bridge::provider_core_chat_tools_from_responses_request(
            &request,
        )
        .ok_or_else(|| {
            format!(
                "invalid_tool_declaration: Gemini request field `tools[{index}]` is not a supported tool declaration"
            )
        })?;
        gemini_validate_openai_tools_with_kind(&Value::Array(converted), kind).map_err(|reason| {
            format!(
                "invalid_tool_declaration: Gemini request field `tools[{index}]` translates to an invalid declaration: {reason}"
            )
        })?;
        verified[index] = b'1';
    }
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
