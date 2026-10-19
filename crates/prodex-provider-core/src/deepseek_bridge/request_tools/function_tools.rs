//! Host acquisition for the shared compiled function-tool policy.
use super::{DeepSeekFunctionToolOperation, function_tool_policy};

pub fn deepseek_provider_core_function_tool_name(tool: &serde_json::Value) -> Option<String> {
    function_tool_policy(
        DeepSeekFunctionToolOperation::Name,
        tool,
        false,
        "DeepSeek",
        64,
        "",
    )
    .expect("compiled Mojo function tool name policy failed")
}

pub fn deepseek_provider_core_tool_name_from_tool_object(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    function_tool_policy(
        DeepSeekFunctionToolOperation::ObjectName,
        &serde_json::Value::Object(object.clone()),
        false,
        "DeepSeek",
        64,
        "",
    )
    .expect("compiled Mojo tool object name policy failed")
}

pub fn deepseek_provider_core_is_generic_function_tool(tool: &serde_json::Value) -> bool {
    function_tool_policy(
        DeepSeekFunctionToolOperation::IsGeneric,
        tool,
        false,
        "DeepSeek",
        64,
        "",
    )
    .expect("compiled Mojo generic function tool policy failed")
}

pub fn deepseek_provider_core_validate_function_name(
    name: &str,
    provider_label: &str,
) -> Result<(), String> {
    deepseek_provider_core_validate_function_name_with_max_bytes(name, provider_label, 64)
}

pub fn deepseek_provider_core_validate_function_name_with_max_bytes(
    name: &str,
    provider_label: &str,
    max_name_bytes: usize,
) -> Result<(), String> {
    function_tool_policy(
        DeepSeekFunctionToolOperation::ValidateName,
        &serde_json::Value::String(name.to_string()),
        false,
        provider_label,
        max_name_bytes,
        "",
    )
}

pub fn deepseek_provider_core_validate_function_parameters(
    tool: &serde_json::Value,
    name: &str,
    provider_label: &str,
) -> Result<(), String> {
    function_tool_policy(
        DeepSeekFunctionToolOperation::ValidateParameters,
        tool,
        false,
        provider_label,
        64,
        name,
    )
}
