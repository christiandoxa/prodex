use crate::{ProviderTransformLoss, ProviderTransformResult};

#[path = "bridge/chat_compat.rs"]
mod chat_compat;

pub use self::chat_compat::{
    provider_core_chat_compatible_created_at, provider_core_chat_compatible_responses_usage,
    provider_core_chat_compatible_responses_value_from_chat_value,
    provider_core_chat_compatible_rtk_wrapped_tool_arguments,
    provider_core_chat_compatible_tool_call_thought_signature,
    provider_core_chat_compatible_validate_top_level_request_shape,
    provider_core_split_flat_namespace_tool_name,
};

pub fn provider_core_lossless_body(result: Option<&ProviderTransformResult>) -> Option<Vec<u8>> {
    let result = result?;
    match result.loss {
        ProviderTransformLoss::Lossless => result.body.clone(),
        ProviderTransformLoss::DegradedButSafe { .. }
        | ProviderTransformLoss::Rejected { .. }
        | ProviderTransformLoss::UnsupportedUpstream { .. } => None,
    }
}

pub fn provider_core_rewritten_body(result: Option<&ProviderTransformResult>) -> Option<Vec<u8>> {
    let result = result?;
    match result.loss {
        ProviderTransformLoss::Lossless | ProviderTransformLoss::DegradedButSafe { .. } => {
            result.body.clone()
        }
        ProviderTransformLoss::Rejected { .. }
        | ProviderTransformLoss::UnsupportedUpstream { .. } => None,
    }
}

pub fn provider_core_rewritten_json_value(
    result: Option<&ProviderTransformResult>,
) -> Option<serde_json::Value> {
    let body = provider_core_rewritten_body(result)?;
    serde_json::from_slice(&body).ok()
}

#[cfg(test)]
#[path = "bridge/tests.rs"]
mod tests;
