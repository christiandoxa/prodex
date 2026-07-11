use super::local_rewrite::RuntimeLocalRewriteProviderOptions;
use super::provider_bridge::{RuntimeProviderBridgeKind, runtime_provider_model_fallback_chain};
pub(super) fn runtime_gemini_model_fallback_chain(
    provider: &RuntimeLocalRewriteProviderOptions,
    requested_model: &str,
) -> Vec<String> {
    if let RuntimeLocalRewriteProviderOptions::Gemini {
        model_resolution, ..
    } = provider
        && let Some(chain) = model_resolution.fallback_chain(requested_model)
    {
        return chain;
    }
    runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Gemini, requested_model)
}
