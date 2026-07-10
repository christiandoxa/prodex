use super::{
    RuntimeLaunchRequest, RuntimeLaunchSelection, runtime_anthropic_api_keys_from_request_or_env,
    runtime_copilot_api_keys_from_request_or_env, runtime_deepseek_api_keys_from_request_or_env,
    runtime_external_provider_oauth_profile_count, runtime_gemini_api_keys_from_request_or_env,
};
use crate::{
    AppState, CodexModelProviderSetting, SUPER_ANTHROPIC_PROVIDER_ID, SUPER_COPILOT_PROVIDER_ID,
    SUPER_DEEPSEEK_PROVIDER_ID, SUPER_GEMINI_PROVIDER_ID, SUPER_KIRO_PROVIDER_ID,
    SUPER_LOCAL_PROVIDER_ID,
};

pub(crate) fn runtime_launch_model_provider_uses_local_rewrite(
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
            .eq_ignore_ascii_case(SUPER_KIRO_PROVIDER_ID)
        || provider
            .provider_id
            .eq_ignore_ascii_case(SUPER_ANTHROPIC_PROVIDER_ID)
        || provider
            .provider_id
            .eq_ignore_ascii_case(SUPER_COPILOT_PROVIDER_ID)
}

pub(crate) fn runtime_local_rewrite_model_provider_id<'a>(
    selection: &'a RuntimeLaunchSelection,
    request: &'a RuntimeLaunchRequest<'_>,
) -> Option<&'a str> {
    request
        .external_provider
        .and_then(runtime_external_provider_local_rewrite_id)
        .or_else(|| {
            selection
                .non_openai_model_provider
                .as_ref()
                .map(|provider| provider.provider_id.as_str())
        })
}

pub(crate) fn runtime_provider_mode_uses_single_api_key(
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
            || provider.eq_ignore_ascii_case("kiro")
    })
}

pub(crate) fn runtime_external_provider_rotation_summary(
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

fn runtime_external_provider_local_rewrite_id(provider: &str) -> Option<&'static str> {
    if provider.eq_ignore_ascii_case("deepseek") {
        Some(SUPER_DEEPSEEK_PROVIDER_ID)
    } else if provider.eq_ignore_ascii_case("gemini")
        || provider.eq_ignore_ascii_case("gemini-oauth")
    {
        Some(SUPER_GEMINI_PROVIDER_ID)
    } else if provider.eq_ignore_ascii_case("anthropic") || provider.eq_ignore_ascii_case("claude")
    {
        Some(SUPER_ANTHROPIC_PROVIDER_ID)
    } else if provider.eq_ignore_ascii_case("copilot")
        || provider.eq_ignore_ascii_case("github-copilot")
        || provider.eq_ignore_ascii_case("github_copilot")
    {
        Some(SUPER_COPILOT_PROVIDER_ID)
    } else if provider.eq_ignore_ascii_case("kiro") {
        Some(SUPER_KIRO_PROVIDER_ID)
    } else {
        None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_mode_detects_explicit_api_key_auth() {
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
    fn local_rewrite_model_provider_id_maps_known_aliases() {
        assert_eq!(
            runtime_external_provider_local_rewrite_id("gemini-oauth"),
            Some(SUPER_GEMINI_PROVIDER_ID)
        );
        assert_eq!(
            runtime_external_provider_local_rewrite_id("github_copilot"),
            Some(SUPER_COPILOT_PROVIDER_ID)
        );
        assert_eq!(
            runtime_external_provider_local_rewrite_id("claude"),
            Some(SUPER_ANTHROPIC_PROVIDER_ID)
        );
        assert_eq!(runtime_external_provider_local_rewrite_id("unknown"), None);
    }
}
