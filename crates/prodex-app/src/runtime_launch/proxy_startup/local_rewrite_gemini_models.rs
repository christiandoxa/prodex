use super::gemini_rewrite::runtime_gemini_request_body_without_tool;
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

pub(super) fn runtime_gemini_unsupported_tool_fallback_body(
    body: &[u8],
) -> Option<(&'static str, Vec<u8>)> {
    ["computerUse", "codeExecution", "urlContext", "googleSearch"]
        .into_iter()
        .find_map(|tool_name| {
            runtime_gemini_request_body_without_tool(body, tool_name).map(|body| (tool_name, body))
        })
}
