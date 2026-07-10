use super::*;

mod copilot_instructions;
mod execution;
mod plan;
mod profile;
mod proxy_args;
mod proxy_startup;

pub(super) use execution::{RuntimeLaunchStrategy, execute_runtime_launch};
use plan::cleanup_runtime_launch_plan;
pub(super) use plan::{ChildProcessPlan, RuntimeLaunchPlan};
#[cfg(test)]
pub(super) use prodex_profile_identity::validate_profile_name;
pub(super) use profile::{
    ensure_path_is_unique, record_run_selection, resolve_profile_name,
    should_enable_runtime_rotation_proxy,
};
#[cfg(test)]
pub(super) use proxy_args::normalize_run_codex_args;
pub(super) use proxy_args::runtime_proxy_codex_passthrough_args;
#[cfg(test)]
pub(super) use proxy_args::{runtime_proxy_codex_args, runtime_proxy_codex_args_with_mount_path};
#[cfg(test)]
pub(super) use proxy_startup::start_runtime_rotation_proxy;
#[cfg(test)]
pub(super) use proxy_startup::start_runtime_rotation_proxy_with_listen_addr;
pub(super) use proxy_startup::{
    RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH, RuntimeAnthropicOAuthProfileAuth,
    RuntimeAnthropicProviderAuth, RuntimeCopilotProfileAuth, RuntimeCopilotProviderAuth,
    RuntimeDeepSeekWebSearchMode, RuntimeGatewayAdminRole, RuntimeGatewayAdminToken,
    RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
    RuntimeGatewayOidcConfig, RuntimeGatewaySsoConfig, RuntimeGatewayStateStore,
    RuntimeGeminiOAuthProfileAuth, RuntimeGeminiProviderAuth, RuntimeKiroProfileAuth,
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
    start_runtime_gateway_rewrite_proxy, start_runtime_local_rewrite_proxy,
};
pub(super) use proxy_startup::{
    RuntimeRotationProxyStartOptions, start_runtime_rotation_proxy_with_options,
};

const OPENAI_CODEX_DEFAULT_CONTEXT_WINDOW_TOKENS: u64 = 128_000;

pub(super) fn runtime_launch_openai_spark_context_codex_args(
    codex_home: &Path,
    args: &[OsString],
) -> Vec<OsString> {
    if !runtime_launch_openai_provider_for_args(codex_home, args) {
        return args.to_vec();
    }
    let Some(model) = runtime_launch_openai_model(codex_home, args) else {
        return args.to_vec();
    };
    if !runtime_launch_openai_model_uses_large_context(&model) {
        return args.to_vec();
    }

    let profile_v2_name = codex_cli_profile_v2_name(args);
    let explicit_context_window =
        runtime_launch_cli_model_context_window_tokens(args).or_else(|| {
            runtime_launch_config_model_context_window_tokens_with_profile_v2(
                codex_home,
                profile_v2_name.as_deref(),
            )
        });
    let context_window = explicit_context_window
        .or_else(|| runtime_launch_openai_model_context_from_models_cache(codex_home, &model))
        .unwrap_or(OPENAI_CODEX_DEFAULT_CONTEXT_WINDOW_TOKENS);
    let mut overrides = Vec::new();
    if explicit_context_window.is_none() {
        overrides.push(format!("model_context_window={context_window}"));
    }
    if runtime_launch_cli_model_auto_compact_token_limit(args)
        .or_else(|| {
            runtime_launch_config_model_auto_compact_token_limit_with_profile_v2(
                codex_home,
                profile_v2_name.as_deref(),
            )
        })
        .is_none()
    {
        overrides.push(format!(
            "model_auto_compact_token_limit={}",
            context_window * 9 / 10
        ));
    }
    if overrides.is_empty() {
        return args.to_vec();
    }

    let mut next_args = Vec::with_capacity(args.len() + overrides.len() * 2);
    for override_entry in overrides {
        next_args.push(OsString::from("-c"));
        next_args.push(OsString::from(override_entry));
    }
    next_args.extend(args.iter().cloned());
    next_args
}

pub(super) fn runtime_launch_cli_model_context_window_tokens(args: &[OsString]) -> Option<u64> {
    codex_cli_config_override_value(args, "model_context_window")
        .as_deref()
        .and_then(runtime_launch_parse_model_context_window_tokens)
}

fn runtime_launch_cli_model_auto_compact_token_limit(args: &[OsString]) -> Option<u64> {
    codex_cli_config_override_value(args, "model_auto_compact_token_limit")
        .as_deref()
        .and_then(runtime_launch_parse_model_context_window_tokens)
}

pub(super) fn runtime_launch_cli_gemini_thinking_budget_tokens(args: &[OsString]) -> Option<u64> {
    codex_cli_config_override_value(args, "model_thinking_budget")
        .as_deref()
        .and_then(runtime_launch_parse_gemini_thinking_budget_tokens)
}

pub(super) fn runtime_launch_config_model_context_window_tokens(codex_home: &Path) -> Option<u64> {
    runtime_launch_config_file_model_context_window_tokens(&codex_home.join("config.toml"))
}

pub(super) fn runtime_launch_config_gemini_thinking_budget_tokens(
    codex_home: &Path,
) -> Option<u64> {
    runtime_launch_config_file_gemini_thinking_budget_tokens(&codex_home.join("config.toml"))
}

pub(super) fn runtime_launch_config_model_context_window_tokens_with_profile_v2(
    codex_home: &Path,
    profile_v2_name: Option<&str>,
) -> Option<u64> {
    profile_v2_name
        .and_then(|profile_v2_name| codex_profile_v2_config_path(codex_home, profile_v2_name))
        .and_then(|config_path| {
            runtime_launch_config_file_model_context_window_tokens(&config_path)
        })
        .or_else(|| runtime_launch_config_model_context_window_tokens(codex_home))
        .or_else(|| {
            runtime_launch_config_model_cache_context_window_tokens_with_profile_v2(
                codex_home,
                profile_v2_name,
            )
        })
}

pub(super) fn runtime_launch_config_model_cache_context_window_tokens_with_profile_v2(
    codex_home: &Path,
    profile_v2_name: Option<&str>,
) -> Option<u64> {
    let provider =
        codex_config_value_with_profile_v2(codex_home, "model_provider", profile_v2_name);
    if provider.is_some_and(|provider| !provider.trim().eq_ignore_ascii_case("openai")) {
        return None;
    }
    let model = codex_config_value_with_profile_v2(codex_home, "model", profile_v2_name)?;
    runtime_launch_openai_model_context_from_models_cache(codex_home, &model)
}

pub(super) fn runtime_launch_config_gemini_thinking_budget_tokens_with_profile_v2(
    codex_home: &Path,
    profile_v2_name: Option<&str>,
) -> Option<u64> {
    profile_v2_name
        .and_then(|profile_v2_name| codex_profile_v2_config_path(codex_home, profile_v2_name))
        .and_then(|config_path| {
            runtime_launch_config_file_gemini_thinking_budget_tokens(&config_path)
        })
        .or_else(|| runtime_launch_config_gemini_thinking_budget_tokens(codex_home))
}

fn runtime_launch_config_model_auto_compact_token_limit_with_profile_v2(
    codex_home: &Path,
    profile_v2_name: Option<&str>,
) -> Option<u64> {
    profile_v2_name
        .and_then(|profile_v2_name| codex_profile_v2_config_path(codex_home, profile_v2_name))
        .and_then(|config_path| {
            runtime_launch_config_file_model_auto_compact_token_limit(&config_path)
        })
        .or_else(|| {
            runtime_launch_config_file_model_auto_compact_token_limit(
                &codex_home.join("config.toml"),
            )
        })
}

fn runtime_launch_config_file_model_context_window_tokens(config_path: &Path) -> Option<u64> {
    let raw = fs::read_to_string(config_path).ok()?;
    let value = toml::from_str::<toml::Value>(&raw).ok()?;
    runtime_launch_toml_model_context_window_tokens(value.get("model_context_window")?)
}

fn runtime_launch_config_file_model_auto_compact_token_limit(config_path: &Path) -> Option<u64> {
    let raw = fs::read_to_string(config_path).ok()?;
    let value = toml::from_str::<toml::Value>(&raw).ok()?;
    runtime_launch_toml_model_context_window_tokens(value.get("model_auto_compact_token_limit")?)
}

fn runtime_launch_config_file_gemini_thinking_budget_tokens(config_path: &Path) -> Option<u64> {
    let raw = fs::read_to_string(config_path).ok()?;
    let value = toml::from_str::<toml::Value>(&raw).ok()?;
    runtime_launch_toml_gemini_thinking_budget_tokens(value.get("model_thinking_budget")?)
}

fn runtime_launch_toml_model_context_window_tokens(value: &toml::Value) -> Option<u64> {
    match value {
        toml::Value::Integer(value) => u64::try_from(*value).ok().filter(|value| *value > 1),
        toml::Value::String(value) => runtime_launch_parse_model_context_window_tokens(value),
        _ => None,
    }
}

fn runtime_launch_toml_gemini_thinking_budget_tokens(value: &toml::Value) -> Option<u64> {
    match value {
        toml::Value::Integer(value) => u64::try_from(*value).ok(),
        toml::Value::String(value) => runtime_launch_parse_gemini_thinking_budget_tokens(value),
        _ => None,
    }
}

fn runtime_launch_parse_model_context_window_tokens(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok().filter(|value| *value > 1)
}

fn runtime_launch_parse_gemini_thinking_budget_tokens(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

fn runtime_launch_openai_provider_for_args(codex_home: &Path, args: &[OsString]) -> bool {
    codex_cli_config_override_value(args, "model_provider")
        .or_else(|| codex_config_value_for_args(codex_home, args, "model_provider"))
        .is_none_or(|provider| provider.trim().eq_ignore_ascii_case("openai"))
}

fn runtime_launch_openai_model(codex_home: &Path, args: &[OsString]) -> Option<String> {
    runtime_launch_cli_model(args)
        .or_else(|| codex_cli_config_override_value(args, "model"))
        .or_else(|| codex_config_value_for_args(codex_home, args, "model"))
}

fn runtime_launch_openai_model_uses_large_context(model: &str) -> bool {
    let model = model.trim().to_ascii_lowercase();
    model.starts_with("gpt-5")
        || matches!(
            model.as_str(),
            "gpt-5.3-codex-spark" | "gpt-5.3-spark" | "codex-auto-review"
        )
}

fn runtime_launch_openai_model_context_from_models_cache(
    codex_home: &Path,
    model: &str,
) -> Option<u64> {
    let raw = match fs::read_to_string(codex_home.join("models_cache.json")) {
        Ok(raw) => raw,
        Err(_) => return None,
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return None;
    };
    let expected = model.trim().to_ascii_lowercase();
    value
        .get("models")
        .and_then(serde_json::Value::as_array)
        .and_then(|models| {
            models.iter().find_map(|entry| {
                let matches_slug = entry
                    .get("slug")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| entry.get("id").and_then(serde_json::Value::as_str))
                    .is_some_and(|slug| slug.trim().eq_ignore_ascii_case(&expected));
                if !matches_slug {
                    return None;
                }
                entry
                    .get("context_window")
                    .or_else(|| entry.get("max_context_window"))
                    .or_else(|| entry.get("max_context_window_tokens"))
                    .and_then(serde_json::Value::as_u64)
                    .filter(|context_window| *context_window > 1)
            })
        })
}

fn runtime_launch_cli_model(args: &[OsString]) -> Option<String> {
    let mut index = 0;
    while index < args.len() {
        let Some(arg) = args[index].to_str() else {
            index += 1;
            continue;
        };
        let model = if matches!(arg, "--model" | "-m") {
            index += 1;
            args.get(index).and_then(|value| value.to_str())
        } else if let Some(value) = arg.strip_prefix("--model=") {
            Some(value)
        } else if let Some(value) = arg.strip_prefix("-m") {
            (!value.is_empty()).then_some(value.trim_start_matches('='))
        } else {
            None
        };
        if let Some(model) = model.filter(|model| !model.trim().is_empty()) {
            return Some(model.to_string());
        }
        index += 1;
    }
    None
}
