//! Gemini thinking-model classification through the shared Mojo config kernel.

use serde_json::Value;

pub fn gemini_provider_core_model_uses_thinking_level(model: &str) -> bool {
    super::gemini_config_value(
        prodex_mojo_core::rich::GeminiConfigKernelOperation::ModelUsesThinkingLevel,
        Some(model),
        None,
        None,
        None,
        None,
    ) == Value::Bool(true)
}
