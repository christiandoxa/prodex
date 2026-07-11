//! DeepSeek tool-shape validation and strict-schema normalization.
//!
//! Pure provider capability checks only; routing, transport, and retry policy stay outside provider-core.

use std::collections::BTreeMap;

mod function_tools;
mod shape;
mod strict_schema;
mod tool_choice;
mod tool_shape;
mod web_search;

pub use self::function_tools::{
    deepseek_provider_core_function_tool_name, deepseek_provider_core_is_generic_function_tool,
    deepseek_provider_core_tool_name_from_tool_object,
    deepseek_provider_core_validate_function_name,
    deepseek_provider_core_validate_function_parameters,
};
use self::strict_schema::deepseek_provider_core_sanitize_strict_schema;
pub use self::tool_choice::{
    deepseek_provider_core_validate_tool_choice_name,
    deepseek_provider_core_validate_tool_choice_shape,
    deepseek_provider_core_validate_tool_choice_target,
};
pub use self::tool_shape::deepseek_provider_core_validate_tools_shape;
pub use self::web_search::{
    deepseek_provider_core_validate_web_search_options,
    deepseek_provider_core_validate_web_search_tool_context_size,
};

pub fn deepseek_provider_core_apply_strict_function_schema(
    tool: &mut serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    let Some(function) = tool
        .get_mut("function")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Ok(());
    };
    let name = function
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("function")
        .to_string();
    let parameters = function
        .entry("parameters".to_string())
        .or_insert_with(|| serde_json::json!({"type": "object"}));
    deepseek_provider_core_sanitize_strict_schema(parameters, &name, provider_label)?;
    function.insert("strict".to_string(), serde_json::Value::Bool(true));
    Ok(())
}

pub fn deepseek_provider_core_dedup_and_validate_function_tools(
    tools: Vec<serde_json::Value>,
    strict_tools: bool,
    provider_label: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let mut seen_tools = BTreeMap::<String, serde_json::Value>::new();
    let mut deduped = Vec::new();
    for mut tool in tools {
        if let Some(name) = deepseek_provider_core_function_tool_name(&tool) {
            deepseek_provider_core_validate_function_name(&name, provider_label)?;
            deepseek_provider_core_validate_function_parameters(&tool, &name, provider_label)?;
            if !strict_tools
                && tool
                    .get("function")
                    .and_then(|function| function.get("strict"))
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
            {
                return Err(format!(
                    "{provider_label} strict function tool `{name}` requires deepseek.strict_tools=true"
                ));
            }
            if strict_tools {
                deepseek_provider_core_apply_strict_function_schema(&mut tool, provider_label)?;
            }
            if let Some(previous) = seen_tools.get(&name) {
                if previous == &tool {
                    continue;
                }
                let previous_generic = deepseek_provider_core_is_generic_function_tool(previous);
                let current_generic = deepseek_provider_core_is_generic_function_tool(&tool);
                if previous_generic && !current_generic {
                    deduped.retain(|deduped_tool: &serde_json::Value| {
                        deepseek_provider_core_function_tool_name(deduped_tool).as_deref()
                            != Some(name.as_str())
                    });
                } else if current_generic {
                    continue;
                } else {
                    return Err(format!(
                        "{provider_label} function tool name `{name}` is duplicated after translation"
                    ));
                }
            }
            seen_tools.insert(name, tool.clone());
        }
        deduped.push(tool);
    }
    Ok(deduped)
}
