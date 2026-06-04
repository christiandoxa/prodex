use super::*;
mod preflight;
mod providers;
use preflight::*;
use providers::*;

struct RunCommandStrategy {
    args: RunArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
    mem_mode: Option<RuntimeMemTranscriptMode>,
    dry_run: bool,
    model_provider_override: Option<String>,
    profile_v2_name: Option<String>,
    model_context_window_tokens: Option<u64>,
}

impl RunCommandStrategy {
    fn new(args: RunArgs) -> Self {
        let (mem_mode, codex_args) = runtime_mem_extract_mode_with_detail(&args.codex_args);
        let (dry_run_arg, codex_args) = extract_prodex_dry_run_flag(&codex_args);
        let (codex_args, include_code_review) =
            prepare_codex_launch_args(&codex_args, args.full_access);
        let model_provider_override =
            codex_cli_config_override_value(&codex_args, "model_provider");
        let profile_v2_name = codex_cli_profile_v2_name(&codex_args);
        let model_context_window_tokens =
            runtime_launch_cli_model_context_window_tokens(&codex_args);
        let dry_run = args.dry_run || dry_run_arg;
        Self {
            args,
            codex_args,
            include_code_review,
            mem_mode,
            dry_run,
            model_provider_override,
            profile_v2_name,
            model_context_window_tokens,
        }
    }
}

impl RuntimeLaunchStrategy for RunCommandStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: !self.args.no_auto_rotate,
            skip_quota_check: self.args.skip_quota_check,
            base_url: self.args.base_url.as_deref(),
            upstream_no_proxy: self.args.no_proxy,
            include_code_review: self.include_code_review,
            smart_context_enabled: false,
            presidio_redaction_enabled: false,
            model_context_window_tokens: self.model_context_window_tokens,
            force_runtime_proxy: false,
            model_provider_override: self.model_provider_override.as_deref(),
            profile_v2_name: self.profile_v2_name.as_deref(),
            external_provider: None,
            external_provider_api_key: None,
        }
    }

    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        if let Some(mem_mode) = self.mem_mode {
            ensure_runtime_mem_prodex_observer(&prepared.paths)?;
            ensure_runtime_mem_codex_watch_for_home_with_mode(&prepared.codex_home, mem_mode)?;
        }
        let codex_args =
            profile_openai_compatible_codex_args(&prepared.codex_home, &self.codex_args);
        let codex_args = prepare_provider_capability_codex_args(&prepared.codex_home, &codex_args)?;
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &codex_args);
        let mut child = codex_child_plan(prepared.codex_home.clone(), runtime_args);
        if self.args.no_proxy && runtime_proxy.is_none() {
            remove_upstream_proxy_env(&mut child);
        }
        Ok(RuntimeLaunchPlan::new(child))
    }
}

pub(super) fn handle_run(args: RunArgs) -> Result<()> {
    if run_launch_route(&args) == RunLaunchRoute::CodexCommandServerDirectPassthrough {
        return handle_codex_command_server_direct_passthrough(args);
    }

    let strategy = RunCommandStrategy::new(args);
    if strategy.dry_run {
        return print_runtime_launch_dry_run(
            "run",
            strategy.runtime_request(),
            RuntimeLaunchDryRunChild::Codex {
                codex_args: strategy.codex_args.clone(),
            },
        );
    }
    execute_runtime_launch(strategy)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunLaunchRoute {
    ManagedRuntime,
    CodexCommandServerDirectPassthrough,
}

fn run_launch_route(args: &RunArgs) -> RunLaunchRoute {
    // Command servers own stdio; keep Prodex preflight/proxy wrapping out of the stream.
    if !args.dry_run && is_codex_command_server_subcommand(&args.codex_args) {
        return RunLaunchRoute::CodexCommandServerDirectPassthrough;
    }
    RunLaunchRoute::ManagedRuntime
}

fn handle_codex_command_server_direct_passthrough(args: RunArgs) -> Result<()> {
    let plan = codex_command_server_direct_passthrough_plan(args)?;
    exit_with_status(run_child_plan(&plan, None)?)
}

fn codex_command_server_direct_passthrough_plan(args: RunArgs) -> Result<ChildProcessPlan> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let selection = RuntimeLaunchSelection::resolve(
        &paths,
        &state,
        args.profile.as_deref(),
        None,
        None,
        None,
        None,
    )?;

    if !selection.profileless_local_home
        && state
            .profiles
            .get(&selection.selected_profile_name)
            .with_context(|| format!("profile '{}' is missing", selection.selected_profile_name))?
            .managed
    {
        prepare_managed_codex_home(&paths, &selection.codex_home)?;
    }

    let codex_args = prodex_runtime_launch::normalize_codex_profile_args(&args.codex_args);
    let mut child = codex_child_plan(selection.codex_home, codex_args);
    if args.no_proxy {
        remove_upstream_proxy_env(&mut child);
    }
    Ok(child)
}

#[derive(Debug, Clone)]
struct RuntimeLaunchSelection {
    initial_profile_name: String,
    selected_profile_name: String,
    codex_home: PathBuf,
    explicit_profile_requested: bool,
    non_openai_model_provider: Option<CodexModelProviderSetting>,
    profileless_local_home: bool,
}

impl RuntimeLaunchSelection {
    fn resolve(
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
        let gemini_external_provider =
            external_provider.is_some_and(|provider| provider.eq_ignore_ascii_case("gemini"));
        let copilot_external_provider = external_provider.is_some_and(|provider| {
            provider.eq_ignore_ascii_case("copilot")
                || provider.eq_ignore_ascii_case("github-copilot")
                || provider.eq_ignore_ascii_case("github_copilot")
        });
        let anthropic_external_provider = external_provider.is_some_and(|provider| {
            provider.eq_ignore_ascii_case("anthropic") || provider.eq_ignore_ascii_case("claude")
        });
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
        )
        .or_else(|| {
            profile_openai_compatible_model_provider_for_launch(
                &codex_home,
                model_provider_override,
            )
        });

        Ok(Self {
            initial_profile_name: profile_name.clone(),
            selected_profile_name: profile_name,
            codex_home,
            explicit_profile_requested: requested.is_some(),
            non_openai_model_provider,
            profileless_local_home: false,
        })
    }

    fn select_profile(
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
        )
        .or_else(|| {
            profile_openai_compatible_model_provider_for_launch(
                &self.codex_home,
                model_provider_override,
            )
        });
        Ok(())
    }
}

fn profile_openai_compatible_model_provider_for_launch(
    codex_home: &Path,
    model_provider_override: Option<&str>,
) -> Option<CodexModelProviderSetting> {
    if model_provider_override.is_some() {
        return None;
    }
    read_profile_openai_compatible_base_url(codex_home).map(|_| CodexModelProviderSetting {
        provider_id: PRODEX_OPENAI_COMPAT_PROVIDER_ID.to_string(),
        source: CodexModelProviderSource::CliOverride,
    })
}

pub(crate) fn resolve_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    let profile_name = resolve_profile_name(state, requested)?;
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
            anyhow::anyhow!(
                "profile '{}' uses {}. `prodex run` currently supports OpenAI/Codex profiles only.",
                profile_name,
                state
                    .profiles
                    .get(&profile_name)
                    .map(|profile| profile.provider.display_name())
                    .unwrap_or("an unsupported provider"),
            )
        })
}

struct RuntimeLaunchPreparationBuilder<'a> {
    request: RuntimeLaunchRequest<'a>,
    paths: AppPaths,
    state: AppState,
    selection: RuntimeLaunchSelection,
}

impl<'a> RuntimeLaunchPreparationBuilder<'a> {
    fn from_request(request: RuntimeLaunchRequest<'a>) -> Result<Self> {
        let paths = AppPaths::discover()?;
        let mut state = AppState::load(&paths)?;
        let selection = select_runtime_launch_profile(&paths, &mut state, &request)?;

        Ok(Self {
            request,
            paths,
            state,
            selection,
        })
    }

    fn build(mut self) -> Result<PreparedRuntimeLaunch> {
        self.record_selection()?;
        self.handle_non_openai_model_provider()?;

        if self.selection.profileless_local_home {
            create_codex_home_if_missing(&self.selection.codex_home)?;
        }

        let managed = self.selected_profile_is_managed()?;
        if managed {
            prepare_managed_codex_home(&self.paths, &self.selection.codex_home)?;
        }

        let runtime_proxy = RuntimeProxyStartupFactory::build(
            &self.paths,
            &self.state,
            &self.selection,
            &self.request,
        )?;

        let RuntimeLaunchPreparationBuilder {
            paths, selection, ..
        } = self;
        Ok(PreparedRuntimeLaunch {
            paths,
            codex_home: selection.codex_home,
            managed,
            runtime_proxy,
        })
    }

    fn handle_non_openai_model_provider(&self) -> Result<()> {
        let Some(setting) = self.selection.non_openai_model_provider.as_ref() else {
            return Ok(());
        };

        if self.request.force_runtime_proxy {
            bail!(
                "profile '{}' uses model_provider '{}' from {}. `prodex claude` requires the default OpenAI/Codex provider.",
                self.selection.selected_profile_name,
                setting.provider_id,
                setting.source.display_name(),
            );
        }

        if local_rewrite_proxy_upstream_base_url(&self.selection, &self.request).is_some() {
            print_wrapped_stderr(&section_header("Runtime Provider"));
            if let Some(provider) = self.request.external_provider {
                let rotation = if provider.eq_ignore_ascii_case("gemini")
                    || provider.eq_ignore_ascii_case("anthropic")
                    || provider.eq_ignore_ascii_case("claude")
                    || provider.eq_ignore_ascii_case("copilot")
                    || provider.eq_ignore_ascii_case("github-copilot")
                    || provider.eq_ignore_ascii_case("github_copilot")
                    || provider.eq_ignore_ascii_case("deepseek")
                {
                    runtime_external_provider_rotation_summary(
                        &self.state,
                        &self.selection.selected_profile_name,
                        provider,
                        self.request.external_provider_api_key,
                        self.request.allow_auto_rotate,
                    )
                } else {
                    "Quota preflight and account rotation stay disabled.".to_string()
                };
                print_wrapped_stderr(&format!(
                    "Using provider '{provider}' through the Smart Context rewrite proxy. {rotation}",
                ));
            } else {
                print_wrapped_stderr(
                    "Using prodex-local through the Smart Context rewrite proxy. Quota preflight and account rotation stay disabled.",
                );
            }
            return Ok(());
        }

        print_wrapped_stderr(&section_header("Runtime Provider"));
        print_wrapped_stderr(&format_runtime_provider_direct_launch_message(
            setting.provider_id.as_str(),
            setting.source.display_name(),
        ));
        Ok(())
    }

    fn record_selection(&mut self) -> Result<()> {
        if self.selection.profileless_local_home {
            return Ok(());
        }

        record_run_selection(&mut self.state, &self.selection.selected_profile_name);
        self.state.save(&self.paths)?;
        Ok(())
    }

    fn selected_profile_is_managed(&self) -> Result<bool> {
        if self.selection.profileless_local_home {
            return Ok(false);
        }

        Ok(self
            .state
            .profiles
            .get(&self.selection.selected_profile_name)
            .with_context(|| {
                format!(
                    "profile '{}' is missing",
                    self.selection.selected_profile_name
                )
            })?
            .managed)
    }
}

struct RuntimeProxyStartupFactory;

impl RuntimeProxyStartupFactory {
    fn build(
        paths: &AppPaths,
        state: &AppState,
        selection: &RuntimeLaunchSelection,
        request: &RuntimeLaunchRequest<'_>,
    ) -> Result<Option<RuntimeProxyEndpoint>> {
        if let Some(local_upstream_base_url) =
            local_rewrite_proxy_upstream_base_url(selection, request)
        {
            return Ok(Some(start_local_rewrite_proxy_endpoint(
                paths,
                state,
                selection,
                request,
                local_upstream_base_url,
            )?));
        }

        if selection.non_openai_model_provider.is_some() {
            return Ok(None);
        }

        let runtime_upstream_base_url = quota_base_url(request.base_url);
        if request.presidio_redaction_enabled || request.smart_context_enabled {
            return Ok(Some(start_dedicated_runtime_proxy_endpoint(
                paths,
                state,
                selection,
                request,
                runtime_upstream_base_url,
            )?));
        }
        if request.force_runtime_proxy && !request.allow_auto_rotate {
            return Ok(Some(start_fixed_runtime_proxy_endpoint(
                paths,
                state,
                selection,
                request,
                runtime_upstream_base_url,
            )?));
        }
        if request.force_runtime_proxy
            || should_enable_runtime_rotation_proxy(
                state,
                &selection.selected_profile_name,
                request.allow_auto_rotate,
            )
        {
            return Ok(Some(ensure_runtime_rotation_proxy_endpoint(
                paths,
                &selection.selected_profile_name,
                runtime_upstream_base_url.as_str(),
                request.include_code_review,
                request.upstream_no_proxy,
                request.smart_context_enabled,
                runtime_launch_effective_model_context_window_tokens(request, selection),
            )?));
        }

        Ok(None)
    }

    fn preview(
        paths: &AppPaths,
        state: &AppState,
        selection: &RuntimeLaunchSelection,
        request: &RuntimeLaunchRequest<'_>,
    ) -> Result<Option<RuntimeProxyEndpoint>> {
        if local_rewrite_proxy_upstream_base_url(selection, request).is_some() {
            return Ok(Some(runtime_local_rewrite_proxy_dry_run_endpoint(
                paths, selection, request,
            )?));
        }

        if selection.non_openai_model_provider.is_some() {
            return Ok(None);
        }

        if request.presidio_redaction_enabled
            || request.force_runtime_proxy
            || request.smart_context_enabled
            || should_enable_runtime_rotation_proxy(
                state,
                &selection.selected_profile_name,
                request.allow_auto_rotate,
            )
        {
            return Ok(Some(runtime_proxy_dry_run_endpoint(paths)?));
        }

        Ok(None)
    }
}

pub(super) fn prepare_runtime_launch(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    RuntimeLaunchPreparationBuilder::from_request(request)?.build()
}

pub(super) fn prepare_runtime_launch_dry_run(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let selection = RuntimeLaunchSelection::resolve(
        &paths,
        &state,
        request.profile,
        request.model_provider_override,
        request.profile_v2_name,
        request.external_provider,
        request.external_provider_api_key,
    )?;
    let managed = if selection.profileless_local_home {
        false
    } else {
        state
            .profiles
            .get(&selection.selected_profile_name)
            .with_context(|| format!("profile '{}' is missing", selection.selected_profile_name))?
            .managed
    };
    let runtime_proxy = RuntimeProxyStartupFactory::preview(&paths, &state, &selection, &request)?;

    Ok(PreparedRuntimeLaunch {
        paths,
        codex_home: selection.codex_home,
        managed,
        runtime_proxy,
    })
}

fn runtime_proxy_dry_run_endpoint(paths: &AppPaths) -> Result<RuntimeProxyEndpoint> {
    Ok(RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:0"
            .parse()
            .context("failed to build dry-run runtime proxy address")?,
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        local_model_provider_id: None,
        lease_dir: paths.root.join("runtime-broker-dry-run-leases"),
        _lease: None,
        _direct_proxy: None,
    })
}

fn runtime_local_rewrite_proxy_dry_run_endpoint(
    paths: &AppPaths,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<RuntimeProxyEndpoint> {
    let local_model_provider_id = runtime_local_rewrite_model_provider_id(selection, request)
        .unwrap_or(SUPER_LOCAL_PROVIDER_ID);
    Ok(RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:0"
            .parse()
            .context("failed to build dry-run runtime local rewrite proxy address")?,
        openai_mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        local_model_provider_id: Some(local_model_provider_id.to_string()),
        lease_dir: paths.root.join("runtime-local-proxy-dry-run-leases"),
        _lease: None,
        _direct_proxy: None,
    })
}

fn start_dedicated_runtime_proxy_endpoint(
    paths: &AppPaths,
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
    runtime_upstream_base_url: String,
) -> Result<RuntimeProxyEndpoint> {
    let model_context_window_tokens =
        runtime_launch_effective_model_context_window_tokens(request, selection);
    let proxy = start_runtime_rotation_proxy_with_options(RuntimeRotationProxyStartOptions {
        paths,
        state,
        current_profile: &selection.selected_profile_name,
        upstream_base_url: runtime_upstream_base_url,
        include_code_review: request.include_code_review,
        upstream_no_proxy: request.upstream_no_proxy,
        smart_context_enabled: request.smart_context_enabled,
        presidio_redaction_enabled: request.presidio_redaction_enabled,
        model_context_window_tokens,
        preferred_listen_addr: None,
    })?;
    Ok(RuntimeProxyEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        local_model_provider_id: None,
        lease_dir: paths.root.join("runtime-dedicated-proxy-leases"),
        _lease: None,
        _direct_proxy: Some(proxy),
    })
}

fn start_fixed_runtime_proxy_endpoint(
    paths: &AppPaths,
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
    runtime_upstream_base_url: String,
) -> Result<RuntimeProxyEndpoint> {
    let model_context_window_tokens =
        runtime_launch_effective_model_context_window_tokens(request, selection);
    let fixed_state = fixed_runtime_proxy_state(state, &selection.selected_profile_name)?;
    let proxy = start_runtime_rotation_proxy_with_options(RuntimeRotationProxyStartOptions {
        paths,
        state: &fixed_state,
        current_profile: &selection.selected_profile_name,
        upstream_base_url: runtime_upstream_base_url,
        include_code_review: request.include_code_review,
        upstream_no_proxy: request.upstream_no_proxy,
        smart_context_enabled: request.smart_context_enabled,
        presidio_redaction_enabled: request.presidio_redaction_enabled,
        model_context_window_tokens,
        preferred_listen_addr: None,
    })?;
    Ok(RuntimeProxyEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        local_model_provider_id: None,
        lease_dir: paths.root.join("runtime-fixed-proxy-leases"),
        _lease: None,
        _direct_proxy: Some(proxy),
    })
}

fn start_local_rewrite_proxy_endpoint(
    paths: &AppPaths,
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
    upstream_base_url: String,
) -> Result<RuntimeProxyEndpoint> {
    let model_context_window_tokens =
        runtime_launch_effective_model_context_window_tokens(request, selection);
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths,
        state,
        upstream_base_url,
        provider: runtime_local_rewrite_provider_options(state, selection, request)?,
        upstream_no_proxy: request.upstream_no_proxy,
        smart_context_enabled: request.smart_context_enabled,
        presidio_redaction_enabled: request.presidio_redaction_enabled,
        model_context_window_tokens,
    })?;
    let local_model_provider_id = runtime_local_rewrite_model_provider_id(selection, request)
        .unwrap_or(SUPER_LOCAL_PROVIDER_ID);
    Ok(RuntimeProxyEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        local_model_provider_id: Some(local_model_provider_id.to_string()),
        lease_dir: paths.root.join("runtime-local-proxy-leases"),
        _lease: None,
        _direct_proxy: Some(proxy),
    })
}

fn local_rewrite_proxy_upstream_base_url(
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
) -> Option<String> {
    if request.force_runtime_proxy || !request.smart_context_enabled {
        return None;
    }
    let provider = selection.non_openai_model_provider.as_ref()?;
    if !runtime_launch_model_provider_uses_local_rewrite(provider) {
        return None;
    }
    request
        .base_url
        .map(str::to_string)
        .or_else(|| {
            codex_config_value_with_profile_v2(
                &selection.codex_home,
                &format!("model_providers.{}.base_url", provider.provider_id.as_str()),
                request.profile_v2_name,
            )
        })
        .filter(|base_url| !base_url.trim().is_empty())
}

fn fixed_runtime_proxy_state(state: &AppState, profile_name: &str) -> Result<AppState> {
    prodex_runtime_launch::fixed_runtime_proxy_state(state, profile_name)
}

fn runtime_launch_effective_model_context_window_tokens(
    request: &RuntimeLaunchRequest<'_>,
    selection: &RuntimeLaunchSelection,
) -> Option<u64> {
    if !request.smart_context_enabled {
        return None;
    }
    request.model_context_window_tokens.or_else(|| {
        runtime_launch_config_model_context_window_tokens_with_profile_v2(
            &selection.codex_home,
            request.profile_v2_name,
        )
    })
}

#[cfg(test)]
#[path = "../../tests/src/app_commands/runtime_launch.rs"]
mod tests;
