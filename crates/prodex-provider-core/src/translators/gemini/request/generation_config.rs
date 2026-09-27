//! Gemini generation-config translation through the shared Mojo request kernel.

#[path = "generation_config/thinking.rs"]
mod thinking;

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
