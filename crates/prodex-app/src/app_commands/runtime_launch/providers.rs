use super::{
    RuntimeLaunchRequest, RuntimeLaunchSelection, active_profile_selection_order,
    resolve_runtime_launch_profile_name, runtime_launch_effective_gemini_thinking_budget_tokens,
    runtime_launch_profile_home,
};
use crate::{
    AppState, CodexModelProviderSetting, ProfileProvider, RuntimeAnthropicOAuthProfileAuth,
    RuntimeAnthropicProviderAuth, RuntimeCopilotProfileAuth, RuntimeCopilotProviderAuth,
    RuntimeGeminiModelResolution, RuntimeGeminiOAuthProfileAuth, RuntimeGeminiProviderAuth,
    RuntimeLocalRewriteProviderOptions, SUPER_ANTHROPIC_PROVIDER_ID, SUPER_COPILOT_PROVIDER_ID,
    SUPER_DEEPSEEK_PROVIDER_ID, SUPER_GEMINI_PROVIDER_ID, SUPER_LOCAL_PROVIDER_ID,
    ensure_gemini_code_assist_project_if_missing, gemini_oauth_project_from_env,
    refresh_claude_oauth_secret_if_needed, resolve_copilot_runtime_api_auth, resolve_profile_name,
};
use anyhow::{Context, Result, bail};
use std::env;
use std::path::PathBuf;

pub(super) fn runtime_launch_should_use_profileless_gemini(
    state: &AppState,
    gemini_external_provider: bool,
    external_provider_api_key: Option<&str>,
) -> bool {
    gemini_external_provider
        && runtime_gemini_api_keys_from_request_or_env(external_provider_api_key).is_some()
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

pub(super) fn resolve_gemini_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    if let Some(requested) = requested {
        return resolve_profile_name(state, Some(requested));
    }
    if let Some(active) = state.active_profile.as_deref()
        && state
            .profiles
            .get(active)
            .is_some_and(|profile| matches!(profile.provider, ProfileProvider::Gemini { .. }))
    {
        return Ok(active.to_string());
    }
    state
        .profiles
        .iter()
        .find_map(|(name, profile)| {
            matches!(profile.provider, ProfileProvider::Gemini { .. }).then(|| name.clone())
        })
        .or_else(|| resolve_runtime_launch_profile_name(state, None).ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "`prodex s --provider gemini` requires a Gemini profile from Google sign-in, or --api-key / GEMINI_API_KEY(S) / GOOGLE_API_KEY(S)"
            )
        })
}

pub(super) fn resolve_copilot_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    if let Some(requested) = requested {
        return resolve_profile_name(state, Some(requested));
    }
    if let Some(active) = state.active_profile.as_deref()
        && state
            .profiles
            .get(active)
            .is_some_and(|profile| matches!(profile.provider, ProfileProvider::Copilot { .. }))
    {
        return Ok(active.to_string());
    }
    state
        .profiles
        .iter()
        .find_map(|(name, profile)| {
            matches!(profile.provider, ProfileProvider::Copilot { .. }).then(|| name.clone())
        })
        .or_else(|| resolve_runtime_launch_profile_name(state, None).ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "`prodex s --provider copilot` requires an imported Copilot profile from `prodex profile import copilot`, or --api-key / GITHUB_COPILOT_API_KEY(S)"
            )
        })
}

pub(super) fn resolve_anthropic_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    if let Some(requested) = requested {
        return resolve_profile_name(state, Some(requested));
    }
    if let Some(active) = state.active_profile.as_deref()
        && state
            .profiles
            .get(active)
            .is_some_and(|profile| matches!(profile.provider, ProfileProvider::Anthropic { .. }))
    {
        return Ok(active.to_string());
    }
    state
        .profiles
        .iter()
        .find_map(|(name, profile)| {
            matches!(profile.provider, ProfileProvider::Anthropic { .. }).then(|| name.clone())
        })
        .or_else(|| resolve_runtime_launch_profile_name(state, None).ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "`prodex s --provider claude` requires a Claude profile from `prodex login --with-claude`, or --api-key / ANTHROPIC_API_KEY(S)"
            )
        })
}

pub(super) fn runtime_launch_profile_home_for_external_provider(
    state: &AppState,
    profile_name: &str,
    external_provider: Option<&str>,
) -> Result<PathBuf> {
    if external_provider.is_some_and(|provider| {
        provider.eq_ignore_ascii_case("gemini")
            || provider.eq_ignore_ascii_case("copilot")
            || provider.eq_ignore_ascii_case("github-copilot")
            || provider.eq_ignore_ascii_case("github_copilot")
            || provider.eq_ignore_ascii_case("anthropic")
            || provider.eq_ignore_ascii_case("claude")
    }) {
        let profile = state
            .profiles
            .get(profile_name)
            .with_context(|| format!("profile '{}' is missing", profile_name))?;
        if matches!(
            profile.provider,
            ProfileProvider::Gemini { .. }
                | ProfileProvider::Copilot { .. }
                | ProfileProvider::Anthropic { .. }
        ) || profile.provider.supports_codex_runtime()
        {
            return Ok(profile.codex_home.clone());
        }
    }
    runtime_launch_profile_home(state, profile_name)
}

pub(super) fn runtime_launch_model_provider_uses_local_rewrite(
    provider: &CodexModelProviderSetting,
) -> bool {
    provider
        .provider_id
        .eq_ignore_ascii_case(SUPER_LOCAL_PROVIDER_ID)
        || provider
            .provider_id
            .eq_ignore_ascii_case(SUPER_DEEPSEEK_PROVIDER_ID)
        || provider
            .provider_id
            .eq_ignore_ascii_case(SUPER_GEMINI_PROVIDER_ID)
        || provider
            .provider_id
            .eq_ignore_ascii_case(SUPER_ANTHROPIC_PROVIDER_ID)
        || provider
            .provider_id
            .eq_ignore_ascii_case(SUPER_COPILOT_PROVIDER_ID)
}

pub(super) fn runtime_local_rewrite_model_provider_id<'a>(
    selection: &'a RuntimeLaunchSelection,
    request: &'a RuntimeLaunchRequest<'_>,
) -> Option<&'a str> {
    request
        .external_provider
        .and_then(|provider| {
            if provider.eq_ignore_ascii_case("deepseek") {
                Some(SUPER_DEEPSEEK_PROVIDER_ID)
            } else if provider.eq_ignore_ascii_case("gemini") {
                Some(SUPER_GEMINI_PROVIDER_ID)
            } else if provider.eq_ignore_ascii_case("anthropic")
                || provider.eq_ignore_ascii_case("claude")
            {
                Some(SUPER_ANTHROPIC_PROVIDER_ID)
            } else if provider.eq_ignore_ascii_case("copilot")
                || provider.eq_ignore_ascii_case("github-copilot")
                || provider.eq_ignore_ascii_case("github_copilot")
            {
                Some(SUPER_COPILOT_PROVIDER_ID)
            } else {
                None
            }
        })
        .or_else(|| {
            selection
                .non_openai_model_provider
                .as_ref()
                .map(|provider| provider.provider_id.as_str())
        })
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
                runtime_anthropic_api_keys_from_request_or_env(request.external_provider_api_key)
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
                runtime_copilot_api_keys_from_request_or_env(request.external_provider_api_key)
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
            )
            .context(
                "DeepSeek provider requires --api-key or DEEPSEEK_API_KEY(S) in the environment",
            )?;
            Ok(RuntimeLocalRewriteProviderOptions::DeepSeek { api_keys })
        }
        Some(provider) if provider.eq_ignore_ascii_case("gemini") => {
            let thinking_budget_tokens =
                runtime_launch_effective_gemini_thinking_budget_tokens(request, selection);
            let model_resolution = RuntimeGeminiModelResolution::from_current_settings();
            if let Some(api_keys) =
                runtime_gemini_api_keys_from_request_or_env(request.external_provider_api_key)
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
        Some(provider) => bail!("unsupported external provider '{provider}'"),
        None => Ok(RuntimeLocalRewriteProviderOptions::OpenAiResponses),
    }
}

pub(super) fn runtime_provider_mode_uses_single_api_key(
    external_provider: Option<&str>,
    external_provider_api_key: Option<&str>,
) -> bool {
    external_provider.is_some_and(|provider| {
        ((provider.eq_ignore_ascii_case("anthropic") || provider.eq_ignore_ascii_case("claude"))
            && runtime_anthropic_api_keys_from_request_or_env(external_provider_api_key).is_some())
            || provider.eq_ignore_ascii_case("deepseek")
            || ((provider.eq_ignore_ascii_case("copilot")
                || provider.eq_ignore_ascii_case("github-copilot")
                || provider.eq_ignore_ascii_case("github_copilot"))
                && runtime_copilot_api_keys_from_request_or_env(external_provider_api_key)
                    .is_some())
            || (provider.eq_ignore_ascii_case("gemini")
                && runtime_gemini_api_keys_from_request_or_env(external_provider_api_key).is_some())
    })
}

pub(super) fn runtime_external_provider_rotation_summary(
    state: &AppState,
    selected_profile_name: &str,
    external_provider: &str,
    external_provider_api_key: Option<&str>,
    allow_auto_rotate: bool,
) -> String {
    if let Some(key_count) =
        runtime_external_provider_api_key_count(external_provider, external_provider_api_key)
    {
        if key_count > 1 {
            return format!(
                "API-key rotation is enabled across {key_count} keys; quota preflight stays disabled."
            );
        }
        return "Using one provider API key; API-key rotation is skipped and quota preflight stays disabled.".to_string();
    }
    if !allow_auto_rotate {
        return "Account rotation is disabled by launch flags; quota preflight stays disabled."
            .to_string();
    }
    let profile_count = runtime_external_provider_oauth_profile_count(
        state,
        selected_profile_name,
        external_provider,
    );
    if profile_count > 1 {
        format!(
            "OAuth account rotation is enabled across {profile_count} profiles for fresh requests; continuations stay pinned to their owning profile and quota preflight stays disabled."
        )
    } else {
        "Using one OAuth profile; account rotation is skipped and quota preflight stays disabled."
            .to_string()
    }
}

fn runtime_external_provider_api_key_count(
    external_provider: &str,
    external_provider_api_key: Option<&str>,
) -> Option<usize> {
    if external_provider.eq_ignore_ascii_case("anthropic")
        || external_provider.eq_ignore_ascii_case("claude")
    {
        return runtime_anthropic_api_keys_from_request_or_env(external_provider_api_key)
            .map(|keys| keys.len());
    }
    if external_provider.eq_ignore_ascii_case("deepseek") {
        return runtime_deepseek_api_keys_from_request_or_env(external_provider_api_key)
            .map(|keys| keys.len());
    }
    if external_provider.eq_ignore_ascii_case("copilot")
        || external_provider.eq_ignore_ascii_case("github-copilot")
        || external_provider.eq_ignore_ascii_case("github_copilot")
    {
        return runtime_copilot_api_keys_from_request_or_env(external_provider_api_key)
            .map(|keys| keys.len());
    }
    if external_provider.eq_ignore_ascii_case("gemini") {
        return runtime_gemini_api_keys_from_request_or_env(external_provider_api_key)
            .map(|keys| keys.len());
    }
    None
}

fn runtime_external_provider_oauth_profile_count(
    state: &AppState,
    selected_profile_name: &str,
    external_provider: &str,
) -> usize {
    active_profile_selection_order(state, selected_profile_name)
        .into_iter()
        .filter(|profile_name| {
            state.profiles.get(profile_name).is_some_and(|profile| {
                runtime_external_provider_matches_profile(external_provider, &profile.provider)
            })
        })
        .count()
}

fn runtime_external_provider_matches_profile(
    external_provider: &str,
    profile_provider: &ProfileProvider,
) -> bool {
    if external_provider.eq_ignore_ascii_case("gemini") {
        return matches!(profile_provider, ProfileProvider::Gemini { .. });
    }
    if external_provider.eq_ignore_ascii_case("anthropic")
        || external_provider.eq_ignore_ascii_case("claude")
    {
        return matches!(profile_provider, ProfileProvider::Anthropic { .. });
    }
    if external_provider.eq_ignore_ascii_case("copilot")
        || external_provider.eq_ignore_ascii_case("github-copilot")
        || external_provider.eq_ignore_ascii_case("github_copilot")
    {
        return matches!(profile_provider, ProfileProvider::Copilot { .. });
    }
    false
}

fn runtime_anthropic_oauth_profiles_for_provider(
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<Vec<RuntimeAnthropicOAuthProfileAuth>> {
    let selected_profile_name = selection.selected_profile_name.as_str();
    let selected_profile = state
        .profiles
        .get(selected_profile_name)
        .with_context(|| format!("profile '{}' is missing", selected_profile_name))?;
    if !matches!(selected_profile.provider, ProfileProvider::Anthropic { .. }) {
        bail!(
            "profile '{}' uses {}. Run `prodex login --with-claude`, or pass --api-key / ANTHROPIC_API_KEY(S).",
            selected_profile_name,
            selected_profile.provider.display_name()
        );
    }

    let profile_names = if request.allow_auto_rotate {
        active_profile_selection_order(state, selected_profile_name)
    } else {
        vec![selected_profile_name.to_string()]
    };
    let mut profiles = Vec::new();
    let mut errors = Vec::new();
    for profile_name in profile_names {
        let Some(profile) = state.profiles.get(&profile_name) else {
            continue;
        };
        let ProfileProvider::Anthropic { .. } = &profile.provider else {
            continue;
        };
        match refresh_claude_oauth_secret_if_needed(&profile.codex_home) {
            Ok(secret) => {
                profiles.push(RuntimeAnthropicOAuthProfileAuth {
                    profile_name: profile_name.clone(),
                    access_token: secret.access_token,
                });
            }
            Err(err) => {
                errors.push(format!("{profile_name}: {err:#}"));
            }
        }
    }

    if profiles.is_empty() {
        let suffix = if errors.is_empty() {
            String::new()
        } else {
            format!(": {}", errors.join("; "))
        };
        bail!("no usable Anthropic Claude OAuth profiles found{suffix}");
    }

    Ok(profiles)
}

fn runtime_copilot_profiles_for_provider(
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<Vec<RuntimeCopilotProfileAuth>> {
    let selected_profile_name = selection.selected_profile_name.as_str();
    let selected_profile = state
        .profiles
        .get(selected_profile_name)
        .with_context(|| format!("profile '{}' is missing", selected_profile_name))?;
    if !matches!(selected_profile.provider, ProfileProvider::Copilot { .. }) {
        bail!(
            "profile '{}' uses {}. Run `prodex profile import copilot`, or pass --api-key / GITHUB_COPILOT_API_KEY(S).",
            selected_profile_name,
            selected_profile.provider.display_name()
        );
    }

    let profile_names = if request.allow_auto_rotate {
        active_profile_selection_order(state, selected_profile_name)
    } else {
        vec![selected_profile_name.to_string()]
    };
    let mut profiles = Vec::new();
    let mut errors = Vec::new();
    for profile_name in profile_names {
        let Some(profile) = state.profiles.get(&profile_name) else {
            continue;
        };
        let ProfileProvider::Copilot { host, login, .. } = &profile.provider else {
            continue;
        };
        match resolve_copilot_runtime_api_auth(host, login) {
            Ok(auth) => profiles.push(RuntimeCopilotProfileAuth {
                profile_name: profile_name.clone(),
                api_key: auth.api_key,
            }),
            Err(err) => errors.push(format!("{profile_name}: {err:#}")),
        }
    }

    if profiles.is_empty() {
        let suffix = if errors.is_empty() {
            String::new()
        } else {
            format!(": {}", errors.join("; "))
        };
        bail!("no usable Copilot profiles found{suffix}");
    }

    Ok(profiles)
}

fn runtime_gemini_oauth_profiles_for_provider(
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<Vec<RuntimeGeminiOAuthProfileAuth>> {
    let selected_profile_name = selection.selected_profile_name.as_str();
    let selected_profile = state
        .profiles
        .get(selected_profile_name)
        .with_context(|| format!("profile '{}' is missing", selected_profile_name))?;
    if !matches!(selected_profile.provider, ProfileProvider::Gemini { .. }) {
        bail!(
            "profile '{}' uses {}. Run `prodex login` and choose Sign in with Google, or pass --api-key / GEMINI_API_KEY(S) / GOOGLE_API_KEY(S).",
            selected_profile_name,
            selected_profile.provider.display_name()
        );
    }

    let profile_names = if request.allow_auto_rotate {
        active_profile_selection_order(state, selected_profile_name)
    } else {
        vec![selected_profile_name.to_string()]
    };
    let mut profiles = Vec::new();
    let mut errors = Vec::new();
    for profile_name in profile_names {
        let Some(profile) = state.profiles.get(&profile_name) else {
            continue;
        };
        let ProfileProvider::Gemini { email, project_id } = &profile.provider else {
            continue;
        };
        match ensure_gemini_code_assist_project_if_missing(&profile.codex_home) {
            Ok(secret) => {
                profiles.push(RuntimeGeminiOAuthProfileAuth {
                    profile_name: profile_name.clone(),
                    codex_home: profile.codex_home.clone(),
                    email: Some(email.clone()),
                    access_token: secret.access_token,
                    project_id: secret
                        .project_id
                        .or_else(gemini_oauth_project_from_env)
                        .or_else(|| project_id.clone()),
                });
            }
            Err(err) => {
                errors.push(format!("{profile_name}: {err:#}"));
            }
        }
    }

    if profiles.is_empty() {
        let suffix = if errors.is_empty() {
            String::new()
        } else {
            format!(": {}", errors.join("; "))
        };
        bail!("no usable Gemini OAuth profiles found{suffix}");
    }

    Ok(profiles)
}

fn runtime_deepseek_api_keys_from_request_or_env(value: Option<&str>) -> Option<Vec<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .or_else(|| {
            env::var("DEEPSEEK_API_KEYS")
                .ok()
                .and_then(|value| runtime_provider_api_keys_from_list(&value))
                .or_else(|| {
                    env::var("DEEPSEEK_API_KEY")
                        .ok()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .map(|value| vec![value])
                })
        })
}

fn runtime_anthropic_api_keys_from_request_or_env(value: Option<&str>) -> Option<Vec<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .or_else(|| {
            env::var("ANTHROPIC_API_KEYS")
                .ok()
                .and_then(|value| runtime_provider_api_keys_from_list(&value))
                .or_else(|| {
                    env::var("ANTHROPIC_API_KEY")
                        .ok()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .map(|value| vec![value])
                })
        })
}

fn runtime_provider_api_keys_from_list(value: &str) -> Option<Vec<String>> {
    let keys = value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!keys.is_empty()).then_some(keys)
}

fn runtime_copilot_api_keys_from_request_or_env(value: Option<&str>) -> Option<Vec<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .or_else(|| {
            env::var("GITHUB_COPILOT_API_KEYS")
                .ok()
                .and_then(|value| runtime_provider_api_keys_from_list(&value))
                .or_else(|| {
                    env::var("GITHUB_COPILOT_API_KEY")
                        .ok()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .map(|value| vec![value])
                })
        })
}

fn runtime_gemini_api_keys_from_request_or_env(value: Option<&str>) -> Option<Vec<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .or_else(|| {
            ["GEMINI_API_KEYS", "GOOGLE_API_KEYS"]
                .into_iter()
                .find_map(|key| {
                    env::var(key)
                        .ok()
                        .and_then(|value| runtime_provider_api_keys_from_list(&value))
                })
                .or_else(|| {
                    ["GEMINI_API_KEY", "GOOGLE_API_KEY"]
                        .into_iter()
                        .find_map(|key| {
                            env::var(key)
                                .ok()
                                .map(|value| value.trim().to_string())
                                .filter(|value| !value.is_empty())
                                .map(|value| vec![value])
                        })
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestEnvVarGuard;

    #[test]
    fn provider_api_key_list_accepts_common_separators() {
        assert_eq!(
            runtime_provider_api_keys_from_list(" one, two;three\nfour ").unwrap(),
            vec!["one", "two", "three", "four"]
        );
        assert!(runtime_provider_api_keys_from_list(" , ; \n ").is_none());
    }

    #[test]
    fn provider_mode_detects_explicit_api_key_auth() {
        let _copilot_keys = TestEnvVarGuard::unset("GITHUB_COPILOT_API_KEYS");
        let _copilot_key = TestEnvVarGuard::unset("GITHUB_COPILOT_API_KEY");
        assert!(runtime_provider_mode_uses_single_api_key(
            Some("anthropic"),
            Some("sk-anthropic")
        ));
        assert!(runtime_provider_mode_uses_single_api_key(
            Some("copilot"),
            Some("copilot-token")
        ));
        assert!(runtime_provider_mode_uses_single_api_key(
            Some("gemini"),
            Some("gemini-token")
        ));
        assert!(!runtime_provider_mode_uses_single_api_key(
            Some("copilot"),
            None
        ));
    }

    #[test]
    fn gemini_api_key_list_reads_plural_env_before_single_env() {
        let _gemini_keys = TestEnvVarGuard::set("GEMINI_API_KEYS", "one,two");
        let _google_keys = TestEnvVarGuard::unset("GOOGLE_API_KEYS");
        let _gemini_key = TestEnvVarGuard::set("GEMINI_API_KEY", "single");
        let _google_key = TestEnvVarGuard::unset("GOOGLE_API_KEY");

        let keys = runtime_gemini_api_keys_from_request_or_env(None).unwrap();

        assert_eq!(keys, vec!["one", "two"]);
    }

    #[test]
    fn copilot_api_key_list_reads_plural_env() {
        let _copilot_keys = TestEnvVarGuard::set("GITHUB_COPILOT_API_KEYS", "one;two");
        let _copilot_key = TestEnvVarGuard::unset("GITHUB_COPILOT_API_KEY");

        let keys = runtime_copilot_api_keys_from_request_or_env(None).unwrap();

        assert_eq!(keys, vec!["one", "two"]);
    }
}
