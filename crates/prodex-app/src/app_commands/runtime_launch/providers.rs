use super::{
    RuntimeLaunchRequest, RuntimeLaunchSelection, active_profile_selection_order,
    resolve_runtime_launch_profile_name, runtime_launch_profile_home,
};
use crate::{
    AppState, CodexModelProviderSetting, ProfileProvider, RuntimeGeminiOAuthProfileAuth,
    RuntimeGeminiProviderAuth, RuntimeLocalRewriteProviderOptions, SUPER_DEEPSEEK_PROVIDER_ID,
    SUPER_GEMINI_PROVIDER_ID, SUPER_LOCAL_PROVIDER_ID, gemini_oauth_project_from_env,
    refresh_gemini_oauth_secret_if_needed, resolve_profile_name,
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
        && runtime_gemini_api_key_from_request_or_env(external_provider_api_key).is_some()
        && !state
            .profiles
            .values()
            .any(|profile| profile.provider.supports_codex_runtime())
        && !state
            .profiles
            .values()
            .any(|profile| matches!(profile.provider, ProfileProvider::Gemini { .. }))
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
                "`prodex s --provider gemini` requires a Gemini profile from Google sign-in, or --api-key / GEMINI_API_KEY"
            )
        })
}

pub(super) fn runtime_launch_profile_home_for_external_provider(
    state: &AppState,
    profile_name: &str,
    external_provider: Option<&str>,
) -> Result<PathBuf> {
    if external_provider.is_some_and(|provider| provider.eq_ignore_ascii_case("gemini")) {
        let profile = state
            .profiles
            .get(profile_name)
            .with_context(|| format!("profile '{}' is missing", profile_name))?;
        if matches!(profile.provider, ProfileProvider::Gemini { .. })
            || profile.provider.supports_codex_runtime()
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
        Some(provider) if provider.eq_ignore_ascii_case("deepseek") => {
            let api_key = runtime_deepseek_api_key_from_request_or_env(
                request.external_provider_api_key,
            )
            .context(
                "DeepSeek provider requires --api-key or DEEPSEEK_API_KEY in the environment",
            )?;
            Ok(RuntimeLocalRewriteProviderOptions::DeepSeek { api_key })
        }
        Some(provider) if provider.eq_ignore_ascii_case("gemini") => {
            if let Some(api_key) =
                runtime_gemini_api_key_from_request_or_env(request.external_provider_api_key)
            {
                return Ok(RuntimeLocalRewriteProviderOptions::Gemini {
                    auth: RuntimeGeminiProviderAuth::ApiKey { api_key },
                });
            }
            if selection.profileless_local_home {
                bail!(
                    "Gemini provider requires Google sign-in from `prodex login`, or --api-key / GEMINI_API_KEY"
                );
            }
            let profiles = runtime_gemini_oauth_profiles_for_provider(state, selection, request)?;
            Ok(RuntimeLocalRewriteProviderOptions::Gemini {
                auth: RuntimeGeminiProviderAuth::OAuthProfiles { profiles },
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
        provider.eq_ignore_ascii_case("deepseek")
            || (provider.eq_ignore_ascii_case("gemini")
                && runtime_gemini_api_key_from_request_or_env(external_provider_api_key).is_some())
    })
}

pub(super) fn runtime_gemini_oauth_rotation_summary(
    state: &AppState,
    selected_profile_name: &str,
) -> String {
    let profile_count = active_profile_selection_order(state, selected_profile_name)
        .into_iter()
        .filter(|profile_name| {
            state
                .profiles
                .get(profile_name)
                .is_some_and(|profile| matches!(profile.provider, ProfileProvider::Gemini { .. }))
        })
        .count();
    if profile_count > 1 {
        format!(
            "Gemini OAuth account rotation is enabled across {profile_count} profiles; quota preflight stays disabled."
        )
    } else {
        "Using the single available Gemini OAuth profile; account rotation is skipped and quota preflight stays disabled.".to_string()
    }
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
            "profile '{}' uses {}. Run `prodex login` and choose Sign in with Google, or pass --api-key / GEMINI_API_KEY.",
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
        match refresh_gemini_oauth_secret_if_needed(&profile.codex_home) {
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

fn runtime_deepseek_api_key_from_request_or_env(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            env::var("DEEPSEEK_API_KEY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn runtime_gemini_api_key_from_request_or_env(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            ["GEMINI_API_KEY", "GOOGLE_API_KEY"]
                .into_iter()
                .find_map(|key| {
                    env::var(key)
                        .ok()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                })
        })
}
