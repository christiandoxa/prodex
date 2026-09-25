//! Supported-parameter reporting for the shared Responses-to-chat bridge.

use crate::ProviderId;
use crate::translator::{ProviderParamSupport, ProviderUnsupportedReason};

pub(crate) fn responses_chat_compat_supported_params(provider: ProviderId) -> ProviderParamSupport {
    let unsupported = prodex_mojo_core::rich::openai_compat_supported_params(provider.label())
        .unwrap_or_else(|error| {
            panic!("Mojo OpenAI compatibility parameter report failed: {error:?}")
        })
        .into_iter()
        .map(|parameter| ProviderUnsupportedReason {
            field: parameter.field,
            reason: parameter.reason,
        })
        .collect();
    ProviderParamSupport {
        supported: true,
        unsupported,
    }
}
