use anyhow::{Context, Result};

pub(super) fn runtime_gemini_harden_tool_call_thought_signatures(
    body: &mut Vec<u8>,
    model: &str,
) -> Result<usize> {
    prodex_provider_core::gemini_provider_core_harden_tool_call_thought_signatures(body, model)
        .context("failed to harden Gemini request tool thought signatures")
}
