//! DeepSeek tool-shape validation and strict-schema normalization.
//!
//! Pure provider capability checks only; routing, transport, and retry policy stay outside provider-core.

use prodex_mojo_core::json::{DeepSeekFunctionToolOperation, deepseek_function_tool_policy};

mod function_tools;
mod tool_choice;
mod tool_shape;
mod web_search;

pub use self::function_tools::{
    deepseek_provider_core_function_tool_name, deepseek_provider_core_is_generic_function_tool,
    deepseek_provider_core_tool_name_from_tool_object,
    deepseek_provider_core_validate_function_name,
    deepseek_provider_core_validate_function_name_with_max_bytes,
    deepseek_provider_core_validate_function_parameters,
};
pub use self::tool_choice::{
    deepseek_provider_core_validate_tool_choice_name,
    deepseek_provider_core_validate_tool_choice_name_with_max_bytes,
    deepseek_provider_core_validate_tool_choice_shape,
    deepseek_provider_core_validate_tool_choice_target,
};
pub use self::tool_shape::deepseek_provider_core_validate_tools_shape;
pub use self::web_search::{
    deepseek_provider_core_validate_web_search_options,
    deepseek_provider_core_validate_web_search_tool_context_size,
};

#[derive(serde::Deserialize)]
struct FunctionToolResult {
    #[serde(default)]
    value: serde_json::Value,
    error: Option<String>,
}

fn function_tool_policy<T: serde::de::DeserializeOwned>(
    operation: DeepSeekFunctionToolOperation,
    input: &serde_json::Value,
    strict: bool,
    provider_label: &str,
    max_name_bytes: usize,
    name: &str,
) -> Result<T, String> {
    let context = serde_json::json!({
        "input": input,
        "provider_label": provider_label,
        "max_name_bytes": max_name_bytes,
        "name": name,
    });
    let mut document = crate::mojo_json::Document::default();
    document.root(&context);
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let output = deepseek_function_tool_policy(&document.nodes, raw, operation, strict)
        .map_err(|error| format!("{provider_label} function-tool Mojo policy failed: {error:?}"))?;
    let output: FunctionToolResult = serde_json::from_slice(&output).map_err(|error| {
        format!("{provider_label} function-tool Mojo policy returned invalid JSON: {error}")
    })?;
    if let Some(error) = output.error {
        return Err(error);
    }
    serde_json::from_value(output.value).map_err(|error| {
        format!("{provider_label} function-tool Mojo policy returned an invalid value: {error}")
    })
}

pub fn deepseek_provider_core_apply_strict_function_schema(
    tool: &mut serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    *tool = function_tool_policy(
        DeepSeekFunctionToolOperation::StrictSchema,
        tool,
        true,
        provider_label,
        64,
        "",
    )?;
    Ok(())
}

pub fn deepseek_provider_core_dedup_and_validate_function_tools(
    tools: Vec<serde_json::Value>,
    strict_tools: bool,
    provider_label: &str,
) -> Result<Vec<serde_json::Value>, String> {
    deepseek_provider_core_dedup_and_validate_function_tools_with_max_bytes(
        tools,
        strict_tools,
        provider_label,
        64,
    )
}

pub fn deepseek_provider_core_dedup_and_validate_function_tools_with_max_bytes(
    tools: Vec<serde_json::Value>,
    strict_tools: bool,
    provider_label: &str,
    max_name_bytes: usize,
) -> Result<Vec<serde_json::Value>, String> {
    function_tool_policy(
        DeepSeekFunctionToolOperation::ValidateAndDeduplicate,
        &serde_json::Value::Array(tools),
        strict_tools,
        provider_label,
        max_name_bytes,
        "",
    )
}

#[cfg(test)]
#[path = "request_tools_tests.rs"]
mod tests;
