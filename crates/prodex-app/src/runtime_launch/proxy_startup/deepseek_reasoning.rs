use super::provider_bridge::RuntimeProviderBridgeKind;
use anyhow::Result;
use prodex_provider_core::{
    deepseek_provider_core_apply_reasoning_from_responses_request,
    deepseek_provider_core_thinking_enabled, deepseek_provider_core_validate_reasoning_shape,
};

pub(super) fn runtime_deepseek_apply_reasoning_from_responses_request(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    deepseek_provider_core_apply_reasoning_from_responses_request(
        value,
        request,
        provider_kind.chat_compatible_adapter_label(),
        provider_kind == RuntimeProviderBridgeKind::Gemini,
    )
    .map_err(anyhow::Error::msg)
}

pub(super) fn runtime_deepseek_thinking_enabled(value: &serde_json::Value) -> bool {
    deepseek_provider_core_thinking_enabled(value)
}

pub(super) fn runtime_deepseek_validate_reasoning_shape_for_provider(
    value: &serde_json::Value,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    deepseek_provider_core_validate_reasoning_shape(
        value,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}
