//! Gemini built-in request tool adapters for the Mojo request kernel.

use serde_json::{Value, json};

/// Classify one full tool array in compiled Mojo rather than issuing a
/// separate SimpleRequest kernel call for each source item.
pub(crate) fn gemini_builtin_tool_indices(tools: &[Value]) -> Vec<usize> {
    if tools.is_empty() {
        return Vec::new();
    }
    let body = serde_json::to_vec(&json!({"tools": tools})).expect("Gemini tool array serializes");
    prodex_mojo_core::provider_constraints::gemini_builtin_tool_positions(&body, tools.len())
        .expect("compiled Mojo Gemini built-in tool batch planner returned invalid output")
}

pub(crate) fn gemini_builtin_tools_from_request(tools: &[Value]) -> Vec<Value> {
    let tools: Vec<_> = gemini_builtin_tool_indices(tools)
        .into_iter()
        .map(|index| {
            let mut tool = tools[index].clone();
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
