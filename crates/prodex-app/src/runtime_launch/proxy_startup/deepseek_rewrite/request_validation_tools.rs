use super::super::{RuntimeDeepSeekRewriteOptions, RuntimeDeepSeekWebSearchMode};
use crate::runtime_launch::proxy_startup::provider_bridge::RuntimeProviderBridgeKind;
use anyhow::Result;
use prodex_provider_core::{
    deepseek_provider_core_dedup_and_validate_function_tools,
    deepseek_provider_core_function_tool_name,
    deepseek_provider_core_validate_tool_choice_name as core_validate_tool_choice_name,
    deepseek_provider_core_validate_tool_choice_shape as core_validate_tool_choice_shape,
    deepseek_provider_core_validate_tool_choice_target as core_validate_tool_choice_target,
    deepseek_provider_core_validate_tools_shape,
    deepseek_provider_core_validate_web_search_options as core_validate_web_search_options,
};
use std::collections::BTreeSet;

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_validate_tools_shape(
    value: &serde_json::Value,
    gemini_compat: bool,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    deepseek_provider_core_validate_tools_shape(
        value,
        gemini_compat,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}

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

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_function_tool_name(
    tool: &serde_json::Value,
) -> Option<String> {
    deepseek_provider_core_function_tool_name(tool)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_validate_tool_choice_name(
    tool_choice: &serde_json::Value,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    core_validate_tool_choice_name(tool_choice, provider_kind.chat_compatible_adapter_label())
        .map_err(anyhow::Error::msg)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_validate_tool_choice_shape(
    value: &serde_json::Value,
    thinking_enabled: bool,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    core_validate_tool_choice_shape(
        value,
        thinking_enabled,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_validate_tool_choice_target(
    tool_choice: &serde_json::Value,
    tool_names: &BTreeSet<String>,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    core_validate_tool_choice_target(
        tool_choice,
        tool_names,
        provider_kind.chat_compatible_adapter_label(),
    )
    .map_err(anyhow::Error::msg)
}

fn runtime_deepseek_validate_web_search_options(
    options: &serde_json::Value,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    core_validate_web_search_options(options, provider_kind.chat_compatible_adapter_label())
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
        RuntimeDeepSeekWebSearchMode::Auto => {
            let _ = request;
            let _ = web_search_options;
            anyhow::bail!(
                "{provider_label} web search auto mode has no documented native OpenAI Chat route yet; set {provider_config_key}.web_search_mode=openai_chat for best-effort forwarding"
            )
        }
        RuntimeDeepSeekWebSearchMode::OpenAiChat => {
            runtime_deepseek_validate_web_search_options(&web_search_options, provider_kind)?;
            request.insert("web_search_options".to_string(), web_search_options);
            Ok(())
        }
        RuntimeDeepSeekWebSearchMode::Off => {
            anyhow::bail!(
                "{provider_label} web search mode is off; remove web_search tools or set {provider_config_key}.web_search_mode"
            )
        }
        RuntimeDeepSeekWebSearchMode::Anthropic => {
            anyhow::bail!(
                "{provider_label} web search mode `anthropic` requires an Anthropic-compatible adapter, which this Responses adapter does not enable yet"
            )
        }
        RuntimeDeepSeekWebSearchMode::FunctionProxy => {
            anyhow::bail!(
                "{provider_label} web search mode `function_proxy` requires a local search backend, which this adapter does not enable yet"
            )
        }
    }
}
