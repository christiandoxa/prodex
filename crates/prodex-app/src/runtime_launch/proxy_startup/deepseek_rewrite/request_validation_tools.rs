use super::super::{RuntimeDeepSeekRewriteOptions, RuntimeDeepSeekWebSearchMode};
use crate::runtime_launch::proxy_startup::provider_bridge::RuntimeProviderBridgeKind;
use anyhow::Result;
use prodex_provider_core::{
    deepseek_provider_core_dedup_and_validate_function_tools,
    deepseek_provider_core_validate_web_search_options as core_validate_web_search_options,
};

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_dedup_and_validate_function_tools(
    tools: Vec<serde_json::Value>,
    options: RuntimeDeepSeekRewriteOptions,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<Vec<serde_json::Value>> {
    deepseek_provider_core_dedup_and_validate_function_tools(
        tools,
        options.strict_tools,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_apply_web_search_mode(
    request: &mut serde_json::Map<String, serde_json::Value>,
    web_search_options: serde_json::Value,
    options: RuntimeDeepSeekRewriteOptions,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    let provider_label = provider_kind.chat_compatible_adapter_label();
    let provider_config_key = provider_kind.provider_id().label();
    match options.web_search_mode {
        RuntimeDeepSeekWebSearchMode::Auto
        | RuntimeDeepSeekWebSearchMode::OpenAiChat
        | RuntimeDeepSeekWebSearchMode::Anthropic => {
            core_validate_web_search_options(&web_search_options, provider_label)
                .map_err(anyhow::Error::msg)?;
            request.insert("web_search_options".to_string(), web_search_options);
            Ok(())
        }
        RuntimeDeepSeekWebSearchMode::Off => {
            anyhow::bail!(
                "{provider_label} web search mode is off; remove web_search tools or set {provider_config_key}.web_search_mode"
            )
        }
    }
}
