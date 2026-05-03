#[allow(unused_imports)]
pub(crate) use runtime_anthropic_crate::{
    RuntimeProxyClaudeModelAlias, RuntimeProxyResponsesModelDescriptor,
    runtime_proxy_claude_additional_model_option_entries, runtime_proxy_claude_alias_env_keys,
    runtime_proxy_claude_alias_model, runtime_proxy_claude_alias_picker_value,
    runtime_proxy_claude_custom_model_option_env, runtime_proxy_claude_managed_model_option_value,
    runtime_proxy_claude_model_override, runtime_proxy_claude_native_client_tool_enabled,
    runtime_proxy_claude_native_computer_enabled, runtime_proxy_claude_native_shell_enabled,
    runtime_proxy_claude_picker_model, runtime_proxy_claude_picker_model_descriptor,
    runtime_proxy_claude_pinned_alias_env, runtime_proxy_claude_reasoning_effort_override,
    runtime_proxy_claude_target_model, runtime_proxy_claude_use_foundry_compat,
    runtime_proxy_normalize_responses_reasoning_effort, runtime_proxy_responses_model_capabilities,
    runtime_proxy_responses_model_descriptor, runtime_proxy_responses_model_descriptors,
    runtime_proxy_responses_model_supported_effort_levels,
    runtime_proxy_responses_model_supports_native_computer_tool,
    runtime_proxy_responses_model_supports_xhigh,
};

#[cfg(test)]
#[path = "../../../../tests/unit/src/runtime_claude/models.rs"]
mod tests;
