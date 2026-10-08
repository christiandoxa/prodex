#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextModelContextWindow {
    pub tokens: u64,
    pub source: &'static str,
    pub registry_version: &'static str,
}

pub const SMART_CONTEXT_MODEL_REGISTRY_VERSION: &str = "smart-context-model-registry-v2";

pub fn smart_context_model_context_window(
    model_name: Option<&str>,
) -> Option<SmartContextModelContextWindow> {
    let normalized = super::smart_context_normalized_model_name(model_name)?;
    let tokens = prodex_mojo_core::smart_context_model_registry::context_window_tokens(&normalized)
        .expect("Mojo Smart Context model registry returned invalid output")?;
    Some(SmartContextModelContextWindow {
        tokens,
        source: "model_registry",
        registry_version: SMART_CONTEXT_MODEL_REGISTRY_VERSION,
    })
}
