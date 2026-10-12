use super::*;

use prodex_mojo_core::rich::{CatalogModel, resolve_catalog_model};
use prodex_mojo_core::super_provider_config::{
    RuntimeOpenAiScalarPolicy, runtime_openai_scalar_policy,
};
pub(crate) use prodex_runtime_launch::runtime_launch_cli_model;

mod copilot_instructions;
mod execution;
mod plan;
mod profile;
mod proxy_args;
mod proxy_startup;

pub(super) use execution::{RuntimeLaunchStrategy, emit_runtime_timing, execute_runtime_launch};
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
pub(super) use proxy_args::{
    runtime_proxy_codex_passthrough_args, runtime_proxy_force_http_responses,
};
#[cfg(test)]
pub(super) use proxy_startup::start_runtime_rotation_proxy;
#[cfg(test)]
pub(super) use proxy_startup::start_runtime_rotation_proxy_with_listen_addr;
pub(super) use proxy_startup::{
    RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH, RuntimeAnthropicOAuthProfileAuth,
    RuntimeAnthropicProviderAuth, RuntimeCopilotProfileAuth, RuntimeCopilotProviderAuth,
    RuntimeDeepSeekWebSearchMode, RuntimeGeminiOAuthProfileAuth, RuntimeGeminiProviderAuth,
    RuntimeKiroProfileAuth, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, RuntimeRotationProxyStartOptions,
    start_runtime_local_rewrite_proxy, start_runtime_rotation_proxy_with_options,
};

/// Native Codex owns model context and automatic compaction defaults.
///
/// This common run/Super boundary deliberately does not read the catalog or
/// synthesize overrides. Explicit CLI/config/profile values retain their native
/// precedence, including malformed values that Codex itself must diagnose.
pub(super) fn runtime_launch_openai_model_context_codex_args(
    _codex_home: &Path,
    args: &[OsString],
) -> Result<Vec<OsString>> {
    Ok(args.to_vec())
}

pub(super) fn runtime_launch_cli_model_context_window_tokens(args: &[OsString]) -> Option<u64> {
    codex_cli_config_override_value(args, "model_context_window")
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
) -> Result<Option<u64>> {
    let explicit = profile_v2_name
        .and_then(|profile_v2_name| codex_profile_v2_config_path(codex_home, profile_v2_name))
        .and_then(|config_path| {
            runtime_launch_config_file_model_context_window_tokens(&config_path)
        })
        .or_else(|| runtime_launch_config_model_context_window_tokens(codex_home));
    if explicit.is_some() {
        return Ok(explicit);
    }
    runtime_launch_config_model_cache_context_window_tokens_with_profile_v2(
        codex_home,
        profile_v2_name,
    )
}

pub(super) fn runtime_launch_config_model_cache_context_window_tokens_with_profile_v2(
    codex_home: &Path,
    profile_v2_name: Option<&str>,
) -> Result<Option<u64>> {
    let provider =
        codex_config_value_with_profile_v2(codex_home, "model_provider", profile_v2_name)?;
    if provider.is_some_and(|provider| {
        !runtime_openai_scalar_policy(RuntimeOpenAiScalarPolicy::ProviderName, &provider)
            .expect("runtime OpenAI provider policy should accept Rust strings")
    }) {
        return Ok(None);
    }
    let Some(model) = codex_config_value_with_profile_v2(codex_home, "model", profile_v2_name)?
    else {
        return Ok(None);
    };
    Ok(runtime_launch_openai_model_context_from_models_cache(
        codex_home, &model,
    ))
}

pub(super) fn runtime_launch_effective_model_context_window_tokens(
    request: &RuntimeLaunchRequest<'_>,
    codex_home: &Path,
) -> Result<Option<u64>> {
    match (
        request.smart_context_enabled,
        request.model_context_window_tokens,
    ) {
        (false, _) => Ok(None),
        (_, Some(value)) => Ok(Some(value)),
        _ => runtime_launch_config_model_context_window_tokens_with_profile_v2(
            codex_home,
            request.profile_v2_name,
        ),
    }
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

fn runtime_launch_config_file_model_context_window_tokens(config_path: &Path) -> Option<u64> {
    runtime_launch_config_file_value(config_path, "model_context_window")
        .as_ref()
        .and_then(runtime_launch_toml_model_context_window_tokens)
}

fn runtime_launch_config_file_gemini_thinking_budget_tokens(config_path: &Path) -> Option<u64> {
    runtime_launch_config_file_value(config_path, "model_thinking_budget")
        .as_ref()
        .and_then(runtime_launch_toml_gemini_thinking_budget_tokens)
}

fn runtime_launch_config_file_value(config_path: &Path, key: &str) -> Option<toml::Value> {
    codex_config_file_toml_value(config_path, key)
        .ok()
        .flatten()
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

fn runtime_launch_openai_model_context_from_models_cache(
    codex_home: &Path,
    model: &str,
) -> Option<u64> {
    let raw = fs::read_to_string(codex_home.join("models_cache.json")).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    let entries = value.get("models").and_then(serde_json::Value::as_array)?;
    let indexed = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            entry
                .get("slug")
                .and_then(serde_json::Value::as_str)
                .or_else(|| entry.get("id").and_then(serde_json::Value::as_str))
                .map(str::trim)
                .filter(|slug| !slug.is_empty())
                .map(|slug| (index, slug))
        })
        .collect::<Vec<_>>();
    let catalog = indexed
        .iter()
        .map(|(_, slug)| CatalogModel {
            id: slug,
            aliases: &[],
        })
        .collect::<Vec<_>>();
    let index = resolve_catalog_model(&catalog, model)
        .expect("Mojo OpenAI models-cache identity lookup failed")?;
    let entry = &entries[indexed[index].0];
    // max_context_window is a supported ceiling, not the active model window.
    // Reading model metadata for proxy budgeting must not promote that ceiling.
    entry
        .get("context_window")
        .and_then(serde_json::Value::as_u64)
        .filter(|context_window| *context_window > 1)
}

#[cfg(test)]
mod openai_model_policy_tests {
    use super::*;

    #[test]
    fn openai_models_cache_uses_mojo_identity_without_promoting_max_context() {
        let root = crate::test_support::test_temp_root().join(format!(
            "prodex-openai-model-cache-policy-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("models_cache.json"),
            serde_json::json!({
                "models": [
                    {
                        "slug": " GPT-5.6-SOL ",
                        "context_window": 200000,
                        "max_context_window": 1000000
                    },
                    {
                        "id": "gpt-5.5",
                        "context_window": 300000,
                        "max_context_window_tokens": 900000
                    }
                ]
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            runtime_launch_openai_model_context_from_models_cache(&root, " gpt-5.6-sol "),
            Some(200000)
        );
        assert_eq!(
            runtime_launch_openai_model_context_from_models_cache(&root, "GPT-5.5"),
            Some(300000)
        );
        assert_eq!(
            runtime_launch_openai_model_context_from_models_cache(&root, "missing"),
            None
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
