use super::{
    RuntimeLaunchRequest, RuntimeLaunchSelection, active_profile_selection_order,
    resolve_runtime_launch_profile_name,
};
use crate::{
    AppState, KIRO_MODEL_CATALOG_FILE, ProfileProvider, RuntimeAnthropicOAuthProfileAuth,
    RuntimeCopilotProfileAuth, RuntimeGeminiOAuthProfileAuth, RuntimeKiroProfileAuth,
    ensure_gemini_code_assist_project_if_missing, gemini_oauth_project_from_env,
    parse_kiro_model_catalog_text, refresh_claude_oauth_secret_if_needed,
    resolve_copilot_runtime_api_auth, resolve_profile_name, write_copilot_runtime_model_catalog,
};
use anyhow::{Context, Result, bail};
use redaction::redaction_redact_secret_like_text;
use std::collections::BTreeSet;

pub(crate) fn resolve_gemini_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    resolve_provider_profile_name(
        state,
        requested,
        |provider| matches!(provider, ProfileProvider::Gemini { .. }),
        "`prodex s --provider gemini` requires a Gemini profile from Google sign-in, or --api-key / GEMINI_API_KEY(S) / GOOGLE_API_KEY(S)",
    )
}

pub(crate) fn resolve_copilot_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    resolve_provider_profile_name(
        state,
        requested,
        |provider| matches!(provider, ProfileProvider::Copilot { .. }),
        "`prodex s --provider copilot` requires an imported Copilot profile from `prodex profile import copilot`, or --api-key / GITHUB_COPILOT_API_KEY(S)",
    )
}

pub(crate) fn resolve_anthropic_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    resolve_provider_profile_name(
        state,
        requested,
        |provider| matches!(provider, ProfileProvider::Anthropic { .. }),
        "`prodex s --provider claude` requires a Claude profile from `prodex login --with-claude`, or --api-key / ANTHROPIC_API_KEY(S)",
    )
}

pub(crate) fn resolve_kiro_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    resolve_provider_profile_name(
        state,
        requested,
        |provider| matches!(provider, ProfileProvider::Kiro { .. }),
        "`prodex super --cli kiro` requires an imported Kiro profile from `prodex profile import kiro`",
    )
}

pub(crate) fn runtime_external_provider_oauth_profile_count(
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

pub(crate) fn runtime_anthropic_oauth_profiles_for_provider(
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
                errors.push(runtime_provider_profile_error(&profile_name, &err));
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

pub(crate) fn runtime_copilot_profiles_for_provider(
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
        let ProfileProvider::Copilot {
            host,
            login,
            api_url,
            ..
        } = &profile.provider
        else {
            continue;
        };
        match resolve_copilot_runtime_api_auth(host, login) {
            Ok(auth) => profiles.push(RuntimeCopilotProfileAuth {
                profile_name: profile_name.clone(),
                api_key: auth.api_key,
                api_url: api_url.clone(),
                model_catalog: auth.model_catalog,
            }),
            Err(err) => errors.push(runtime_provider_profile_error(&profile_name, &err)),
        }
    }

    if !profiles.is_empty() {
        let catalog = runtime_copilot_combined_model_catalog(&profiles);
        if !catalog.is_empty() {
            write_copilot_runtime_model_catalog(&selection.codex_home, &catalog)?;
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

pub(crate) fn runtime_gemini_oauth_profiles_for_provider(
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
                        .or_else(|| project_id.clone())
                        .or_else(gemini_oauth_project_from_env),
                });
            }
            Err(err) => {
                errors.push(runtime_provider_profile_error(&profile_name, &err));
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

pub(crate) fn runtime_kiro_gateway_profile_auth(
    state: &AppState,
) -> Result<RuntimeKiroProfileAuth> {
    let profile_name = resolve_kiro_runtime_launch_profile_name(state, None)?;
    runtime_kiro_profile_auth(state, &profile_name)
}

pub(crate) fn runtime_kiro_profile_for_provider(
    state: &AppState,
    selection: &RuntimeLaunchSelection,
) -> Result<RuntimeKiroProfileAuth> {
    runtime_kiro_profile_auth(state, selection.selected_profile_name.as_str())
}

fn resolve_provider_profile_name(
    state: &AppState,
    requested: Option<&str>,
    matches_provider: impl Fn(&ProfileProvider) -> bool,
    missing_message: &str,
) -> Result<String> {
    if let Some(requested) = requested {
        return resolve_profile_name(state, Some(requested));
    }
    if let Some(active) = state.active_profile.as_deref()
        && state
            .profiles
            .get(active)
            .is_some_and(|profile| matches_provider(&profile.provider))
    {
        return Ok(active.to_string());
    }
    state
        .profiles
        .iter()
        .find_map(|(name, profile)| matches_provider(&profile.provider).then(|| name.clone()))
        .or_else(|| resolve_runtime_launch_profile_name(state, None).ok())
        .ok_or_else(|| anyhow::anyhow!("{}", missing_message))
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
    if external_provider.eq_ignore_ascii_case("kiro") {
        return matches!(profile_provider, ProfileProvider::Kiro { .. });
    }
    false
}

fn runtime_copilot_combined_model_catalog(
    profiles: &[RuntimeCopilotProfileAuth],
) -> Vec<serde_json::Value> {
    let mut seen = BTreeSet::new();
    let mut catalog = Vec::new();
    for profile in profiles {
        for model in &profile.model_catalog {
            let Some(id) = model.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let id = id.trim();
            if id.is_empty() || !seen.insert(id.to_ascii_lowercase()) {
                continue;
            }
            catalog.push(model.clone());
        }
    }
    catalog
}

fn runtime_kiro_profile_auth(
    state: &AppState,
    selected_profile_name: &str,
) -> Result<RuntimeKiroProfileAuth> {
    let selected_profile = state
        .profiles
        .get(selected_profile_name)
        .with_context(|| format!("profile '{}' is missing", selected_profile_name))?;
    if !matches!(selected_profile.provider, ProfileProvider::Kiro { .. }) {
        bail!(
            "profile '{}' uses {}. Run `prodex profile import kiro` first.",
            selected_profile_name,
            selected_profile.provider.display_name()
        );
    }
    let model_catalog =
        std::fs::read_to_string(selected_profile.codex_home.join(KIRO_MODEL_CATALOG_FILE))
            .ok()
            .and_then(|text| parse_kiro_model_catalog_text(&text).ok())
            .unwrap_or_default();
    Ok(RuntimeKiroProfileAuth {
        profile_name: selected_profile_name.to_string(),
        codex_home: selected_profile.codex_home.clone(),
        model_catalog,
        command: None,
    })
}

fn runtime_provider_profile_error(profile_name: &str, err: &anyhow::Error) -> String {
    format!(
        "{}: {}",
        profile_name,
        redaction_redact_secret_like_text(&format!("{err:#}"))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_provider_profile_error_redacts_secret_like_chain() {
        let err = anyhow::anyhow!("failed: Authorization: Bearer provider-profile-token")
            .context("provider profile refresh failed");

        let message = runtime_provider_profile_error("main", &err);

        assert!(message.contains("main: provider profile refresh failed"));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(!message.contains("provider-profile-token"));
    }
}
