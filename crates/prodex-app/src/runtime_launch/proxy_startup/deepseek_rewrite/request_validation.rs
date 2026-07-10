use super::{RuntimeDeepSeekRewriteOptions, RuntimeDeepSeekWebSearchMode};
use crate::runtime_launch::proxy_startup::provider_bridge::RuntimeProviderBridgeKind;
use anyhow::Result;
use prodex_provider_core::{
    deepseek_provider_core_ensure_json_prompt_instruction,
    deepseek_provider_core_insert_primitive_request_fields, deepseek_provider_core_json_string,
    deepseek_provider_core_note_thinking_tool_choice_omission,
    deepseek_provider_core_reject_beta_completion_fields,
    deepseek_provider_core_reject_unsupported_request_fields,
    deepseek_provider_core_response_format_from_responses_request,
    deepseek_provider_core_response_metadata_from_responses_request,
    deepseek_provider_core_stop_from_responses_request,
    deepseek_provider_core_top_logprobs_from_responses_request,
    deepseek_provider_core_user_id_from_responses_request,
};
#[path = "request_validation_tools.rs"]
mod request_validation_tools;
pub(in crate::runtime_launch::proxy_startup) use self::request_validation_tools::{
    runtime_deepseek_apply_web_search_mode, runtime_deepseek_dedup_and_validate_function_tools,
    runtime_deepseek_function_tool_name, runtime_deepseek_validate_tool_choice_name,
    runtime_deepseek_validate_tool_choice_shape, runtime_deepseek_validate_tool_choice_target,
    runtime_deepseek_validate_tools_shape,
};
use std::collections::BTreeMap;

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_stop_from_responses_request(
    value: &serde_json::Value,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<Option<serde_json::Value>> {
    deepseek_provider_core_stop_from_responses_request(
        value,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_insert_primitive_request_fields(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    deepseek_provider_core_insert_primitive_request_fields(
        value,
        request,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_top_logprobs_from_responses_request(
    value: &serde_json::Value,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<Option<serde_json::Value>> {
    deepseek_provider_core_top_logprobs_from_responses_request(
        value,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_reject_unsupported_request_fields(
    value: &serde_json::Value,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    deepseek_provider_core_reject_unsupported_request_fields(
        value,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_reject_beta_completion_fields(
    value: &serde_json::Value,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    deepseek_provider_core_reject_beta_completion_fields(
        value,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_response_format_from_responses_request(
    value: &serde_json::Value,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<Option<serde_json::Value>> {
    deepseek_provider_core_response_format_from_responses_request(
        value,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_response_metadata_from_responses_request(
    value: &serde_json::Value,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<Option<serde_json::Value>> {
    deepseek_provider_core_response_metadata_from_responses_request(
        value,
        provider_kind.chat_compatible_adapter_label(),
        provider_kind.provider_id().label(),
    )
    .map_err(anyhow::Error::msg)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_note_thinking_tool_choice_omission(
    value: &serde_json::Value,
    thinking_enabled: bool,
    provider_kind: RuntimeProviderBridgeKind,
    response_metadata: &mut Option<serde_json::Value>,
) {
    deepseek_provider_core_note_thinking_tool_choice_omission(
        value,
        thinking_enabled,
        provider_kind.chat_compatible_adapter_label(),
        provider_kind.provider_id().label(),
        response_metadata,
    );
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_ensure_json_prompt_instruction(
    messages: &mut Vec<serde_json::Value>,
) {
    deepseek_provider_core_ensure_json_prompt_instruction(messages);
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_user_id_from_responses_request(
    value: &serde_json::Value,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<Option<String>> {
    deepseek_provider_core_user_id_from_responses_request(
        value,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}
