use super::config::runtime_proxy_claude_config_value;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeProxyClaudeModelAlias {
    Opus,
    Sonnet,
    Haiku,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeProxyResponsesModelDescriptor {
    pub(crate) id: &'static str,
    pub(crate) display_name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) claude_alias: Option<RuntimeProxyClaudeModelAlias>,
    pub(crate) claude_picker_model: Option<&'static str>,
    pub(crate) supports_xhigh: bool,
}

pub(crate) fn runtime_proxy_claude_model_override() -> Option<String> {
    env::var("PRODEX_CLAUDE_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn runtime_proxy_normalize_responses_reasoning_effort(
    effort: &str,
) -> Option<&'static str> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "minimal" => Some("minimal"),
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("xhigh"),
        "none" => Some("none"),
        // Claude Code exposes `max`; treat it as the strongest explicit upstream tier.
        "max" => Some("xhigh"),
        _ => None,
    }
}

pub(crate) fn runtime_proxy_claude_reasoning_effort_override() -> Option<String> {
    env::var("PRODEX_CLAUDE_REASONING_EFFORT")
        .ok()
        .and_then(|value| {
            runtime_proxy_normalize_responses_reasoning_effort(value.trim()).map(str::to_string)
        })
}

pub(crate) fn runtime_proxy_claude_native_client_tool_enabled(enabled_tokens: &[&str]) -> bool {
    let Some(value) = env::var("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS").ok() else {
        return false;
    };
    let mut enabled = false;
    for token in value
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        match token.to_ascii_lowercase().as_str() {
            "0" | "false" | "no" | "off" | "none" => return false,
            "1" | "true" | "yes" | "on" | "all" => enabled = true,
            value if enabled_tokens.contains(&value) => enabled = true,
            _ => {}
        }
    }
    enabled
}

pub(crate) fn runtime_proxy_claude_native_shell_enabled() -> bool {
    runtime_proxy_claude_native_client_tool_enabled(&["shell", "bash"])
}

pub(crate) fn runtime_proxy_claude_native_computer_enabled() -> bool {
    runtime_proxy_claude_native_client_tool_enabled(&["computer"])
}

pub(crate) fn runtime_proxy_claude_launch_model(codex_home: &Path) -> String {
    runtime_proxy_claude_model_override()
        .or_else(|| runtime_proxy_claude_config_value(codex_home, "model"))
        .unwrap_or_else(|| DEFAULT_PRODEX_CLAUDE_MODEL.to_string())
}

pub(crate) fn runtime_proxy_claude_alias_env_keys(
    alias: RuntimeProxyClaudeModelAlias,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match alias {
        RuntimeProxyClaudeModelAlias::Opus => (
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES",
        ),
        RuntimeProxyClaudeModelAlias::Sonnet => (
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES",
        ),
        RuntimeProxyClaudeModelAlias::Haiku => (
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_SUPPORTED_CAPABILITIES",
        ),
    }
}

pub(crate) fn runtime_proxy_claude_alias_model(
    alias: RuntimeProxyClaudeModelAlias,
) -> &'static RuntimeProxyResponsesModelDescriptor {
    runtime_proxy_responses_model_descriptors()
        .iter()
        .find(|descriptor| descriptor.claude_alias == Some(alias))
        .expect("Claude alias model should exist")
}

pub(crate) fn runtime_proxy_claude_picker_model_descriptor(
    picker_model: &str,
) -> Option<&'static RuntimeProxyResponsesModelDescriptor> {
    let normalized = picker_model.trim();
    let without_extended_context = normalized.strip_suffix("[1m]").unwrap_or(normalized);
    runtime_proxy_responses_model_descriptor(without_extended_context).or_else(|| {
        runtime_proxy_responses_model_descriptors()
            .iter()
            .find(|descriptor| {
                descriptor
                    .claude_picker_model
                    .is_some_and(|value| value.eq_ignore_ascii_case(without_extended_context))
                    || descriptor.claude_alias.is_some_and(|alias| {
                        runtime_proxy_claude_alias_picker_value(alias)
                            .eq_ignore_ascii_case(without_extended_context)
                    })
            })
    })
}

pub(crate) fn runtime_proxy_responses_model_descriptor(
    model_id: &str,
) -> Option<&'static RuntimeProxyResponsesModelDescriptor> {
    runtime_proxy_responses_model_descriptors()
        .iter()
        .find(|descriptor| descriptor.id.eq_ignore_ascii_case(model_id))
}

pub(crate) fn runtime_proxy_responses_model_capabilities(model_id: &str) -> &'static str {
    if runtime_proxy_responses_model_supports_xhigh(model_id) {
        "effort,max_effort,thinking,adaptive_thinking,interleaved_thinking"
    } else {
        "effort,thinking,adaptive_thinking,interleaved_thinking"
    }
}

pub(crate) fn runtime_proxy_responses_model_supported_effort_levels(
    model_id: &str,
) -> &'static [&'static str] {
    if runtime_proxy_responses_model_supports_xhigh(model_id) {
        &["low", "medium", "high", "max"]
    } else {
        &["low", "medium", "high"]
    }
}

pub(crate) fn runtime_proxy_responses_model_supports_xhigh(model_id: &str) -> bool {
    runtime_proxy_responses_model_descriptor(model_id)
        .map(|descriptor| descriptor.supports_xhigh)
        .unwrap_or_else(|| {
            matches!(
                model_id.trim().to_ascii_lowercase().as_str(),
                value
                    if value.starts_with("gpt-5.2")
                        || value.starts_with("gpt-5.3")
                        || value.starts_with("gpt-5.4")
            )
        })
}

pub(crate) fn runtime_proxy_claude_use_foundry_compat() -> bool {
    true
}

pub(crate) fn runtime_proxy_claude_alias_picker_value(
    alias: RuntimeProxyClaudeModelAlias,
) -> &'static str {
    match alias {
        RuntimeProxyClaudeModelAlias::Opus => "opus",
        RuntimeProxyClaudeModelAlias::Sonnet => "sonnet",
        RuntimeProxyClaudeModelAlias::Haiku => "haiku",
    }
}

pub(crate) fn runtime_proxy_claude_pinned_alias_env() -> Vec<(&'static str, OsString)> {
    let mut env = Vec::new();
    for alias in [
        RuntimeProxyClaudeModelAlias::Opus,
        RuntimeProxyClaudeModelAlias::Sonnet,
        RuntimeProxyClaudeModelAlias::Haiku,
    ] {
        let descriptor = runtime_proxy_claude_alias_model(alias);
        let (model_key, name_key, description_key, caps_key) =
            runtime_proxy_claude_alias_env_keys(alias);
        env.push((model_key, OsString::from(descriptor.id)));
        env.push((name_key, OsString::from(descriptor.id)));
        env.push((description_key, OsString::from(descriptor.description)));
        env.push((
            caps_key,
            OsString::from(runtime_proxy_responses_model_capabilities(descriptor.id)),
        ));
    }
    env
}

pub(crate) fn runtime_proxy_claude_picker_model(target_model: &str) -> String {
    runtime_proxy_responses_model_descriptor(target_model)
        .map(|descriptor| {
            if runtime_proxy_claude_use_foundry_compat() {
                descriptor
                    .claude_alias
                    .map(runtime_proxy_claude_alias_picker_value)
                    .unwrap_or(descriptor.id)
            } else {
                descriptor.id
            }
        })
        .unwrap_or(target_model)
        .to_string()
}

pub(crate) fn runtime_proxy_claude_custom_model_option_env(
    target_model: &str,
) -> Vec<(&'static str, OsString)> {
    if runtime_proxy_responses_model_descriptor(target_model).is_some() {
        return Vec::new();
    }

    let descriptor = runtime_proxy_responses_model_descriptor(target_model);
    let display_name = descriptor
        .map(|descriptor| descriptor.display_name)
        .unwrap_or(target_model);
    let description = descriptor
        .map(|descriptor| descriptor.description.to_string())
        .unwrap_or_else(|| format!("Custom OpenAI model routed through prodex ({target_model})"));

    vec![
        (
            "ANTHROPIC_CUSTOM_MODEL_OPTION",
            OsString::from(target_model),
        ),
        (
            "ANTHROPIC_CUSTOM_MODEL_OPTION_NAME",
            OsString::from(display_name),
        ),
        (
            "ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION",
            OsString::from(description),
        ),
    ]
}

pub(crate) fn runtime_proxy_claude_additional_model_option_entries() -> Vec<serde_json::Value> {
    runtime_proxy_responses_model_descriptors()
        .iter()
        .filter(|descriptor| {
            !(runtime_proxy_claude_use_foundry_compat() && descriptor.claude_alias.is_some())
        })
        .map(|descriptor| {
            let supported_effort_levels =
                runtime_proxy_responses_model_supported_effort_levels(descriptor.id);
            serde_json::json!({
                "value": descriptor.id,
                "label": descriptor.id,
                "description": descriptor.description,
                "supportsEffort": true,
                "supportedEffortLevels": supported_effort_levels,
            })
        })
        .collect()
}

pub(crate) fn runtime_proxy_claude_managed_model_option_value(value: &str) -> bool {
    runtime_proxy_claude_picker_model_descriptor(value).is_some()
}

pub(crate) fn runtime_proxy_responses_model_descriptors()
-> &'static [RuntimeProxyResponsesModelDescriptor] {
    &[
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.4",
            display_name: "GPT-5.4",
            description: "Latest frontier agentic coding model.",
            claude_alias: Some(RuntimeProxyClaudeModelAlias::Opus),
            claude_picker_model: Some("claude-opus-4-6"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.4-mini",
            display_name: "GPT-5.4 Mini",
            description: "Smaller frontier agentic coding model.",
            claude_alias: Some(RuntimeProxyClaudeModelAlias::Haiku),
            claude_picker_model: Some("claude-haiku-4-5"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.3-codex",
            display_name: "GPT-5.3 Codex",
            description: "Frontier Codex-optimized agentic coding model.",
            claude_alias: Some(RuntimeProxyClaudeModelAlias::Sonnet),
            claude_picker_model: Some("claude-sonnet-4-6"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.2-codex",
            display_name: "GPT-5.2 Codex",
            description: "Frontier agentic coding model.",
            claude_alias: None,
            claude_picker_model: Some("claude-sonnet-4-5"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.2",
            display_name: "GPT-5.2",
            description: "Optimized for professional work and long-running agents.",
            claude_alias: None,
            claude_picker_model: Some("claude-opus-4-5"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.1-codex-max",
            display_name: "GPT-5.1 Codex Max",
            description: "Codex-optimized model for deep and fast reasoning.",
            claude_alias: None,
            claude_picker_model: Some("claude-opus-4-1"),
            supports_xhigh: false,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.1-codex-mini",
            display_name: "GPT-5.1 Codex Mini",
            description: "Optimized for Codex. Cheaper, faster, but less capable.",
            claude_alias: None,
            claude_picker_model: Some("claude-haiku-4"),
            supports_xhigh: false,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5",
            display_name: "GPT-5",
            description: "General-purpose GPT-5 model.",
            claude_alias: None,
            claude_picker_model: Some("claude-sonnet-4"),
            supports_xhigh: false,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5-mini",
            display_name: "GPT-5 Mini",
            description: "Smaller GPT-5 model for fast, lower-cost tasks.",
            claude_alias: None,
            claude_picker_model: Some("claude-haiku-3-5"),
            supports_xhigh: false,
        },
    ]
}

pub(crate) fn runtime_proxy_claude_target_model(requested_model: &str) -> String {
    if let Some(override_model) = runtime_proxy_claude_model_override() {
        return override_model;
    }

    let normalized = requested_model.trim();
    if let Some(descriptor) = runtime_proxy_responses_model_descriptor(normalized) {
        return descriptor.id.to_string();
    }
    if let Some(descriptor) = runtime_proxy_claude_picker_model_descriptor(normalized) {
        return descriptor.id.to_string();
    }
    let lower = normalized.to_ascii_lowercase();
    if lower.starts_with("gpt-")
        || lower.starts_with("o1")
        || lower.starts_with("o3")
        || lower.starts_with("o4")
        || lower.contains("codex")
    {
        normalized.to_string()
    } else if lower == "best" || lower == "default" || lower.contains("opus") {
        runtime_proxy_claude_alias_model(RuntimeProxyClaudeModelAlias::Opus)
            .id
            .to_string()
    } else if lower.contains("sonnet") {
        runtime_proxy_claude_alias_model(RuntimeProxyClaudeModelAlias::Sonnet)
            .id
            .to_string()
    } else if lower.contains("haiku") {
        runtime_proxy_claude_alias_model(RuntimeProxyClaudeModelAlias::Haiku)
            .id
            .to_string()
    } else {
        DEFAULT_PRODEX_CLAUDE_MODEL.to_string()
    }
}

pub(crate) fn runtime_proxy_responses_model_supports_native_computer_tool(model_id: &str) -> bool {
    model_id.trim().eq_ignore_ascii_case("gpt-5.4")
}
