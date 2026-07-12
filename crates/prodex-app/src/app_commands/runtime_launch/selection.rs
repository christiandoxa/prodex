use super::*;

#[derive(Debug, Clone)]
pub(super) struct RuntimeLaunchSelection {
    pub(super) initial_profile_name: String,
    pub(super) selected_profile_name: String,
    pub(super) codex_home: PathBuf,
    pub(super) explicit_profile_requested: bool,
    pub(super) non_openai_model_provider: Option<CodexModelProviderSetting>,
    pub(super) profileless_local_home: bool,
}

impl RuntimeLaunchSelection {
    pub(super) fn resolve(
        paths: &AppPaths,
        state: &AppState,
        requested: Option<&str>,
        model_provider_override: Option<&str>,
        profile_v2_name: Option<&str>,
        external_provider: Option<&str>,
        external_provider_api_key: Option<&str>,
    ) -> Result<Self> {
        let profileless_model_provider = codex_non_openai_model_provider_with_profile_v2(
            &paths.shared_codex_root,
            model_provider_override,
            profile_v2_name,
        );
        let gemini_external_provider = external_provider.is_some_and(|provider| {
            provider.eq_ignore_ascii_case("gemini") || provider.eq_ignore_ascii_case("gemini-oauth")
        });
        let copilot_external_provider = external_provider.is_some_and(|provider| {
            provider.eq_ignore_ascii_case("copilot")
                || provider.eq_ignore_ascii_case("github-copilot")
                || provider.eq_ignore_ascii_case("github_copilot")
        });
        let anthropic_external_provider = external_provider.is_some_and(|provider| {
            provider.eq_ignore_ascii_case("anthropic") || provider.eq_ignore_ascii_case("claude")
        });
        let kiro_external_provider =
            external_provider.is_some_and(|provider| provider.eq_ignore_ascii_case("kiro"));
        if requested.is_none()
            && (state.profiles.is_empty()
                || runtime_launch_should_use_profileless_gemini(
                    state,
                    gemini_external_provider,
                    external_provider_api_key,
                )
                || runtime_launch_should_use_profileless_external_provider(
                    state,
                    external_provider,
                    external_provider_api_key,
                ))
            && profileless_model_provider
                .as_ref()
                .is_some_and(runtime_launch_model_provider_uses_local_rewrite)
        {
            let codex_home = paths.shared_codex_root.clone();
            return Ok(Self {
                initial_profile_name: "local".to_string(),
                selected_profile_name: "local".to_string(),
                codex_home,
                explicit_profile_requested: false,
                non_openai_model_provider: profileless_model_provider,
                profileless_local_home: true,
            });
        }

        let profile_name = if gemini_external_provider {
            resolve_gemini_runtime_launch_profile_name(state, requested)?
        } else if copilot_external_provider {
            resolve_copilot_runtime_launch_profile_name(state, requested)?
        } else if anthropic_external_provider {
            resolve_anthropic_runtime_launch_profile_name(state, requested)?
        } else if kiro_external_provider {
            resolve_kiro_runtime_launch_profile_name(state, requested)?
        } else {
            resolve_runtime_launch_profile_name(state, requested)?
        };
        let codex_home = runtime_launch_profile_home_for_external_provider(
            state,
            &profile_name,
            external_provider,
        )?;
        let non_openai_model_provider = codex_non_openai_model_provider_with_profile_v2(
            &codex_home,
            model_provider_override,
            profile_v2_name,
        );
        let non_openai_model_provider = match non_openai_model_provider {
            Some(provider) => Some(provider),
            None => profile_openai_compatible_model_provider_for_launch(
                &codex_home,
                model_provider_override,
            )?,
        };

        Ok(Self {
            initial_profile_name: profile_name.clone(),
            selected_profile_name: profile_name,
            codex_home,
            explicit_profile_requested: requested.is_some(),
            non_openai_model_provider,
            profileless_local_home: false,
        })
    }

    pub(super) fn select_profile(
        &mut self,
        state: &AppState,
        profile_name: &str,
        model_provider_override: Option<&str>,
        profile_v2_name: Option<&str>,
    ) -> Result<()> {
        self.codex_home = runtime_launch_profile_home(state, profile_name)?;
        self.selected_profile_name = profile_name.to_string();
        self.non_openai_model_provider = codex_non_openai_model_provider_with_profile_v2(
            &self.codex_home,
            model_provider_override,
            profile_v2_name,
        );
        if self.non_openai_model_provider.is_none() {
            self.non_openai_model_provider = profile_openai_compatible_model_provider_for_launch(
                &self.codex_home,
                model_provider_override,
            )?;
        }
        Ok(())
    }
}

fn profile_openai_compatible_model_provider_for_launch(
    codex_home: &Path,
    model_provider_override: Option<&str>,
) -> Result<Option<CodexModelProviderSetting>> {
    if model_provider_override.is_some() {
        return Ok(None);
    }
    Ok(
        read_profile_openai_compatible_base_url(codex_home)?.map(|_| CodexModelProviderSetting {
            provider_id: PRODEX_OPENAI_COMPAT_PROVIDER_ID.to_string(),
            source: CodexModelProviderSource::CliOverride,
        }),
    )
}

pub(crate) fn resolve_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    let profile_name = match resolve_profile_name(state, requested) {
        Ok(profile_name) => profile_name,
        Err(_) if requested.is_none() => {
            return active_profile_selection_order(state, "")
                .into_iter()
                .find(|candidate_name| {
                    state
                        .profiles
                        .get(candidate_name)
                        .is_some_and(|profile| profile.provider.supports_codex_runtime())
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "no active profile selected and no Codex-compatible profiles are available; use `prodex use --profile <name>` or pass --profile"
                    )
                });
        }
        Err(err) => return Err(err),
    };
    if requested.is_some() {
        return Ok(profile_name);
    }

    if state
        .profiles
        .get(&profile_name)
        .is_some_and(|profile| profile.provider.supports_codex_runtime())
    {
        return Ok(profile_name);
    }

    active_profile_selection_order(state, &profile_name)
        .into_iter()
        .find(|candidate_name| {
            state.profiles.get(candidate_name).is_some_and(|profile| {
                profile.codex_home.exists() && profile.provider.supports_codex_runtime()
            })
        })
        .ok_or_else(|| {
            let (display_name, route_policy) = state
                .profiles
                .get(&profile_name)
                .map(|profile| {
                    (
                        profile.provider.display_name(),
                        profile.provider.capabilities().runtime_route_policy.label(),
                    )
                })
                .unwrap_or(("an unsupported provider", "unsupported"));
            anyhow::anyhow!(
                "profile '{}' uses {} (route {}). `prodex run` currently supports native OpenAI/Codex profiles only; provider adapters are launched through `prodex s --provider <provider>`.",
                profile_name,
                display_name,
                route_policy,
            )
        })
}
