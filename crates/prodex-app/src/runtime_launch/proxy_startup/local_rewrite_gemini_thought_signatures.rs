use super::gemini_rewrite::RuntimeGeminiTranslatedRequest;
use super::gemini_thought_signatures::runtime_gemini_harden_tool_call_thought_signatures;
use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::provider_bridge::{RuntimeProviderBridgeKind, runtime_provider_label};
use crate::runtime_proxy_log;
use anyhow::Result;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};

pub(super) fn runtime_gemini_harden_translated_thoughts(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile_name: &str,
    translated: &mut RuntimeGeminiTranslatedRequest,
) -> Result<usize> {
    let injected_signatures = runtime_gemini_harden_tool_call_thought_signatures(
        &mut translated.body,
        &translated.model,
    )?;
    if injected_signatures > 0 {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_gemini_synthetic_thought_signature",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field(
                        "provider",
                        runtime_provider_label(RuntimeProviderBridgeKind::Gemini),
                    ),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("model", translated.model.as_str()),
                    runtime_proxy_log_field("count", injected_signatures.to_string()),
                ],
            ),
        );
    }
    Ok(injected_signatures)
}
