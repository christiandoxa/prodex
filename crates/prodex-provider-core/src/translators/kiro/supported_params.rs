//! Kiro supported-parameter reporting.

use crate::translator::ProviderParamSupport;
use prodex_mojo_core::rich::{KiroKernelInput, KiroKernelOperation, kiro_kernel};

fn kiro_supported_params(surface: u64, ignores_required_token_limit: bool) -> ProviderParamSupport {
    let mut input = KiroKernelInput::new(KiroKernelOperation::SupportedParams);
    input.request_id = surface;
    input.include_role = ignores_required_token_limit;
    serde_json::from_slice(
        &kiro_kernel(input)
            .unwrap_or_else(|error| panic!("Mojo Kiro supported-params policy failed: {error:?}")),
    )
    .expect("Mojo Kiro supported-params policy returned invalid JSON")
}

pub(super) fn kiro_chat_completions_supported_params() -> ProviderParamSupport {
    kiro_supported_params(1, false)
}

pub(super) fn kiro_responses_supported_params(
    ignores_required_token_limit: bool,
) -> ProviderParamSupport {
    kiro_supported_params(2, ignores_required_token_limit)
}
