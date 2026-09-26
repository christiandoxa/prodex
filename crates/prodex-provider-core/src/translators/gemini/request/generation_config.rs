//! Gemini generation-config translation through the shared Mojo request kernel.

#[path = "generation_config/thinking.rs"]
mod thinking;

#[cfg(not(feature = "mojo"))]
use prodex_mojo_core::provider_constraints::{
    GeminiBridgeRequestKernelInput, GeminiBridgeRequestOperation, gemini_bridge_request_kernel,
};
use serde_json::Value;

pub use self::thinking::gemini_provider_core_model_uses_thinking_level;

pub(crate) fn gemini_config_value(
    operation: prodex_mojo_core::rich::GeminiConfigKernelOperation,
    primary: Option<&str>,
    secondary: Option<&str>,
    tertiary: Option<&str>,
    quaternary: Option<&str>,
    number: Option<u64>,
) -> Value {
    let mut input = prodex_mojo_core::rich::GeminiConfigKernelInput::new(operation);
    input.primary = primary;
    input.secondary = secondary;
    input.tertiary = tertiary;
    input.quaternary = quaternary;
    input.number = number;
    let body = prodex_mojo_core::rich::gemini_config_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo Gemini config kernel failed: {error:?}"));
    serde_json::from_slice(&body)
        .unwrap_or_else(|error| panic!("Mojo Gemini config kernel returned invalid JSON: {error}"))
}

#[cfg(not(feature = "mojo"))]
pub(crate) fn gemini_validate_candidate_count(value: &Value) -> Result<(), String> {
    let source = serde_json::to_vec(value)
        .map_err(|error| format!("failed to serialize Gemini candidate-count input: {error}"))?;
    match gemini_request_mojo_value(GeminiBridgeRequestKernelInput {
        operation: GeminiBridgeRequestOperation::ValidateCandidateCount,
        primary: Some(&source),
        ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::ValidateCandidateCount)
    })? {
        Value::Null => Ok(()),
        Value::String(error) => Err(error),
        _ => Err("invalid_candidate_count: Mojo returned an invalid validation result".to_string()),
    }
}

#[cfg(not(feature = "mojo"))]
fn gemini_request_mojo_value(input: GeminiBridgeRequestKernelInput<'_>) -> Result<Value, String> {
    let body = gemini_bridge_request_kernel(input)
        .map_err(|error| format!("Mojo Gemini bridge request kernel failed: {error:?}"))?;
    serde_json::from_slice(&body).map_err(|error| {
        format!("Mojo Gemini bridge request kernel returned invalid JSON: {error}")
    })
}
