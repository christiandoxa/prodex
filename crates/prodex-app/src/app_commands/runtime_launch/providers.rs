use super::{
    RuntimeLaunchRequest, RuntimeLaunchSelection, active_profile_selection_order,
    resolve_runtime_launch_profile_name, runtime_launch_effective_gemini_thinking_budget_tokens,
    runtime_launch_profile_home,
};
#[path = "providers_env.rs"]
mod providers_env;
#[path = "providers_mode.rs"]
mod providers_mode;
#[path = "providers_profiles.rs"]
mod providers_profiles;
pub(super) use self::providers_env::{
    runtime_anthropic_api_keys_from_request_or_env, runtime_copilot_api_keys_from_request_or_env,
    runtime_deepseek_api_keys_from_request_or_env, runtime_deepseek_beta_base_url,
    runtime_deepseek_strict_tools_enabled, runtime_deepseek_web_search_mode,
    runtime_gemini_api_keys_from_request_or_env,
};
pub(super) use self::providers_mode::{
    runtime_external_provider_rotation_summary, runtime_launch_model_provider_uses_local_rewrite,
    runtime_local_rewrite_model_provider_id, runtime_provider_mode_uses_single_api_key,
};
pub(super) use self::providers_profiles::{
    resolve_anthropic_runtime_launch_profile_name, resolve_copilot_runtime_launch_profile_name,
    resolve_gemini_runtime_launch_profile_name, resolve_kiro_runtime_launch_profile_name,
    runtime_anthropic_oauth_profiles_for_provider, runtime_copilot_profiles_for_provider,
    runtime_external_provider_oauth_profile_count, runtime_gemini_oauth_profiles_for_provider,
    runtime_kiro_gateway_profile_auth, runtime_kiro_profile_for_provider,
};
use crate::{
    AppState, ProfileProvider, RuntimeAnthropicProviderAuth, RuntimeCopilotProviderAuth,
    RuntimeGeminiModelResolution, RuntimeGeminiProviderAuth, RuntimeLocalRewriteProviderOptions,
};
use anyhow::{Context, Result, bail};
use std::path::PathBuf;

pub(super) fn runtime_launch_should_use_profileless_gemini(
    state: &AppState,
    gemini_external_provider: bool,
    external_provider_api_key: Option<&str>,
) -> bool {
    gemini_external_provider
        && runtime_gemini_api_keys_from_request_or_env(external_provider_api_key)
            .ok()
            .flatten()
            .is_some()
        && !state
            .profiles
            .values()
            .any(|profile| profile.provider.supports_codex_runtime())
        && !state
            .profiles
            .values()
            .any(|profile| matches!(profile.provider, ProfileProvider::Gemini { .. }))
}

pub(super) fn runtime_launch_should_use_profileless_external_provider(
    state: &AppState,
    external_provider: Option<&str>,
    external_provider_api_key: Option<&str>,
) -> bool {
    let Some(provider) = external_provider else {
        return false;
    };
    if !runtime_provider_mode_uses_single_api_key(Some(provider), external_provider_api_key) {
        return false;
    }
    !state
        .profiles
        .values()
        .any(|profile| profile.provider.supports_codex_runtime())
}

pub(super) fn runtime_launch_profile_home_for_external_provider(
    state: &AppState,
    profile_name: &str,
    external_provider: Option<&str>,
) -> Result<PathBuf> {
    if external_provider.is_some_and(|provider| {
        provider.eq_ignore_ascii_case("gemini")
            || provider.eq_ignore_ascii_case("gemini-oauth")
            || provider.eq_ignore_ascii_case("copilot")
            || provider.eq_ignore_ascii_case("github-copilot")
            || provider.eq_ignore_ascii_case("github_copilot")
            || provider.eq_ignore_ascii_case("anthropic")
            || provider.eq_ignore_ascii_case("claude")
            || provider.eq_ignore_ascii_case("kiro")
    }) {
        let profile = state
            .profiles
            .get(profile_name)
            .with_context(|| format!("profile '{}' is missing", profile_name))?;
        if matches!(
            profile.provider,
            ProfileProvider::Gemini { .. }
                | ProfileProvider::Copilot { .. }
                | ProfileProvider::Kiro { .. }
                | ProfileProvider::Anthropic { .. }
                | ProfileProvider::Agy { .. }
        ) || profile.provider.supports_codex_runtime()
        {
            return Ok(profile.codex_home.clone());
        }
    }
    runtime_launch_profile_home(state, profile_name)
}

pub(super) fn runtime_local_rewrite_provider_options(
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<RuntimeLocalRewriteProviderOptions> {
    match request.external_provider {
        Some(provider)
            if provider.eq_ignore_ascii_case("anthropic")
                || provider.eq_ignore_ascii_case("claude") =>
        {
            if let Some(api_keys) =
                runtime_anthropic_api_keys_from_request_or_env(request.external_provider_api_key)?
            {
                return Ok(RuntimeLocalRewriteProviderOptions::Anthropic {
                    auth: RuntimeAnthropicProviderAuth::ApiKeys { api_keys },
                });
            }
            if selection.profileless_local_home {
                bail!(
                    "Anthropic provider requires Claude sign-in from `prodex login --with-claude`, or --api-key / ANTHROPIC_API_KEY(S)"
                );
            }
            let profiles =
                runtime_anthropic_oauth_profiles_for_provider(state, selection, request)?;
            Ok(RuntimeLocalRewriteProviderOptions::Anthropic {
                auth: RuntimeAnthropicProviderAuth::OAuthProfiles { profiles },
            })
        }
        Some(provider)
            if provider.eq_ignore_ascii_case("copilot")
                || provider.eq_ignore_ascii_case("github-copilot")
                || provider.eq_ignore_ascii_case("github_copilot") =>
        {
            if let Some(api_keys) =
                runtime_copilot_api_keys_from_request_or_env(request.external_provider_api_key)?
            {
                return Ok(RuntimeLocalRewriteProviderOptions::Copilot {
                    auth: RuntimeCopilotProviderAuth::ApiKeys { api_keys },
                });
            }
            if selection.profileless_local_home {
                bail!(
                    "Copilot provider requires `prodex profile import copilot`, or --api-key / GITHUB_COPILOT_API_KEY(S)"
                );
            }
            let profiles = runtime_copilot_profiles_for_provider(state, selection, request)?;
            Ok(RuntimeLocalRewriteProviderOptions::Copilot {
                auth: RuntimeCopilotProviderAuth::Profiles { profiles },
            })
        }
        Some(provider) if provider.eq_ignore_ascii_case("deepseek") => {
            let api_keys = runtime_deepseek_api_keys_from_request_or_env(
                request.external_provider_api_key,
            )?
            .context(
                "DeepSeek provider requires --api-key or DEEPSEEK_API_KEY(S) in the environment",
            )?;
            Ok(RuntimeLocalRewriteProviderOptions::DeepSeek {
                api_keys,
                strict_tools: runtime_deepseek_strict_tools_enabled(&selection.codex_home)?,
                beta_base_url: runtime_deepseek_beta_base_url(&selection.codex_home)?,
                web_search_mode: runtime_deepseek_web_search_mode(&selection.codex_home)?,
            })
        }
        Some(provider)
            if provider.eq_ignore_ascii_case("gemini")
                || provider.eq_ignore_ascii_case("gemini-oauth") =>
        {
            let thinking_budget_tokens =
                runtime_launch_effective_gemini_thinking_budget_tokens(request, selection);
            let model_resolution = RuntimeGeminiModelResolution::from_current_settings();
            if !provider.eq_ignore_ascii_case("gemini-oauth")
                && let Some(api_keys) =
                    runtime_gemini_api_keys_from_request_or_env(request.external_provider_api_key)?
            {
                return Ok(RuntimeLocalRewriteProviderOptions::Gemini {
                    auth: RuntimeGeminiProviderAuth::ApiKeys { api_keys },
                    thinking_budget_tokens,
                    model_resolution,
                });
            }
            if selection.profileless_local_home {
                bail!(
                    "Gemini provider requires Google sign-in from `prodex login`, or --api-key / GEMINI_API_KEY(S) / GOOGLE_API_KEY(S)"
                );
            }
            let profiles = runtime_gemini_oauth_profiles_for_provider(state, selection, request)?;
            Ok(RuntimeLocalRewriteProviderOptions::Gemini {
                auth: RuntimeGeminiProviderAuth::OAuthProfiles { profiles },
                thinking_budget_tokens,
                model_resolution,
            })
        }
        Some(provider) if provider.eq_ignore_ascii_case("kiro") => {
            let auth = runtime_kiro_profile_for_provider(state, selection)?;
            Ok(RuntimeLocalRewriteProviderOptions::Kiro { auth })
        }
        Some(provider) => bail!("unsupported external provider '{provider}'"),
        None => Ok(RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: Vec::new(),
        }),
    }
}
