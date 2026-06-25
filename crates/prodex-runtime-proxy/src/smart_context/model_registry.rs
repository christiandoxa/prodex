#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextModelContextWindow {
    pub tokens: u64,
    pub source: &'static str,
    pub registry_version: &'static str,
}

pub const SMART_CONTEXT_MODEL_REGISTRY_VERSION: &str = "smart-context-model-registry-v1";

pub fn smart_context_model_context_window(
    model_name: Option<&str>,
) -> Option<SmartContextModelContextWindow> {
    let normalized = super::smart_context_normalized_model_name(model_name)?;
    let registry_key = normalized.to_ascii_lowercase();
    let tokens = smart_context_registered_model_context_window_tokens(&registry_key)?;
    Some(SmartContextModelContextWindow {
        tokens,
        source: "model_registry",
        registry_version: SMART_CONTEXT_MODEL_REGISTRY_VERSION,
    })
}

fn smart_context_registered_model_context_window_tokens(model: &str) -> Option<u64> {
    match model {
        "unsloth/qwen3.5-35b-a3b" => Some(16_384),
        "gpt-5.3-codex" | "gpt-5.4" | "gpt-5.5" => Some(1_000_000),
        "gpt-5.1-codex" => Some(200_000),
        "claude-sonnet-4-6" => Some(1_000_000),
        "deepseek-v4-pro" => Some(1_048_576),
        _ if model.starts_with("claude-") => Some(200_000),
        _ if model.starts_with("gemini-") => Some(1_048_576),
        _ if model.starts_with("deepseek-") => Some(1_048_576),
        _ => None,
    }
}
