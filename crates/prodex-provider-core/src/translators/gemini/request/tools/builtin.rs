//! Gemini built-in request tool adapters for the Mojo request kernel.

use serde_json::{Value, json};

fn gemini_tool_is_supported_builtin(tool: &Value) -> bool {
    let body = serde_json::to_vec(&json!({"input":"","tools":[tool]}))
        .expect("Gemini built-in tool serializes");
    crate::gemini_bridge::gemini_provider_core_simple_request(&body)
}

pub(crate) fn gemini_builtin_tools_from_request(tools: &[Value]) -> Vec<Value> {
    let tools: Vec<_> = tools
        .iter()
        .filter(|tool| gemini_tool_is_supported_builtin(tool))
        .map(|tool| {
            let mut tool = (*tool).clone();
            if let Some(object) = tool.as_object_mut() {
                object.remove("function");
            }
            tool
        })
        .collect();
    if tools.is_empty() {
        return Vec::new();
    }

    let original =
        serde_json::to_vec(&json!({"tools":tools})).expect("Gemini built-in tools serialize");
    let mut input = prodex_mojo_core::provider_constraints::GeminiBridgeRequestKernelInput::new(
        prodex_mojo_core::provider_constraints::GeminiBridgeRequestOperation::RawTranslatorRequest,
    );
    input.primary = Some(&original);
    input.tertiary = Some(b"[]");
    input.senary = Some(b"\"gemini-2.5-pro\"");
    let body = prodex_mojo_core::provider_constraints::gemini_bridge_request_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo Gemini built-in tool kernel failed: {error:?}"));
    serde_json::from_slice::<Value>(&body)
        .unwrap_or_else(|error| {
            panic!("Mojo Gemini built-in tool kernel returned invalid JSON: {error}")
        })
        .pointer("/request/tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn gemini_is_supported_builtin_tool(tool: &Value) -> bool {
    gemini_tool_is_supported_builtin(tool)
}
