use super::*;
use crate::command_dispatch::command_exit_error;

struct RunCommandStrategy {
    args: RunArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
    mem_mode: Option<RuntimeMemTranscriptMode>,
    dry_run: bool,
    model_provider_override: Option<String>,
}

impl RunCommandStrategy {
    fn new(args: RunArgs) -> Self {
        let (mem_mode, codex_args) = runtime_mem_extract_mode_with_detail(&args.codex_args);
        let (dry_run_arg, codex_args) = extract_prodex_dry_run_flag(&codex_args);
        let (codex_args, include_code_review) =
            prepare_codex_launch_args(&codex_args, args.full_access);
        let model_provider_override =
            codex_cli_config_override_value(&codex_args, "model_provider");
        let dry_run = args.dry_run || dry_run_arg;
        Self {
            args,
            codex_args,
            include_code_review,
            mem_mode,
            dry_run,
            model_provider_override,
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
            force_runtime_proxy: false,
            model_provider_override: self.model_provider_override.as_deref(),
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
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &self.codex_args);
        let mut child = codex_child_plan(prepared.codex_home.clone(), runtime_args);
        if self.args.no_proxy && runtime_proxy.is_none() {
            remove_upstream_proxy_env(&mut child);
        }
        Ok(RuntimeLaunchPlan::new(child))
    }
}

pub(super) fn handle_run(args: RunArgs) -> Result<()> {
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
    ) -> Result<Self> {
        if prodex_runtime_launch::allow_profileless_local_home(
            requested,
            model_provider_override,
            SUPER_LOCAL_PROVIDER_ID,
        ) && state.profiles.is_empty()
        {
            let codex_home = paths.shared_codex_root.clone();
            return Ok(Self {
                initial_profile_name: "local".to_string(),
                selected_profile_name: "local".to_string(),
                codex_home,
                explicit_profile_requested: false,
                non_openai_model_provider: model_provider_override.map(|provider_id| {
                    CodexModelProviderSetting {
                        provider_id: provider_id.to_string(),
                        source: CodexModelProviderSource::CliOverride,
                    }
                }),
                profileless_local_home: true,
            });
        }

        let profile_name = resolve_runtime_launch_profile_name(state, requested)?;
        let codex_home = runtime_launch_profile_home(state, &profile_name)?;
        let non_openai_model_provider =
            codex_non_openai_model_provider(&codex_home, model_provider_override);

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
    ) -> Result<()> {
        self.codex_home = runtime_launch_profile_home(state, profile_name)?;
        self.selected_profile_name = profile_name.to_string();
        self.non_openai_model_provider =
            codex_non_openai_model_provider(&self.codex_home, model_provider_override);
        Ok(())
    }
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
        if selection.non_openai_model_provider.is_some() {
            return Ok(None);
        }

        let runtime_upstream_base_url = quota_base_url(request.base_url);
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
        if selection.non_openai_model_provider.is_some() {
            return Ok(None);
        }

        if request.force_runtime_proxy
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
        lease_dir: paths.root.join("runtime-broker-dry-run-leases"),
        _lease: None,
        _direct_proxy: None,
    })
}

fn start_fixed_runtime_proxy_endpoint(
    paths: &AppPaths,
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
    runtime_upstream_base_url: String,
) -> Result<RuntimeProxyEndpoint> {
    let fixed_state = fixed_runtime_proxy_state(state, &selection.selected_profile_name)?;
    let proxy = start_runtime_rotation_proxy_with_listen_addr(
        paths,
        &fixed_state,
        &selection.selected_profile_name,
        runtime_upstream_base_url,
        request.include_code_review,
        request.upstream_no_proxy,
        None,
    )?;
    Ok(RuntimeProxyEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        lease_dir: paths.root.join("runtime-fixed-proxy-leases"),
        _lease: None,
        _direct_proxy: Some(proxy),
    })
}

fn fixed_runtime_proxy_state(state: &AppState, profile_name: &str) -> Result<AppState> {
    prodex_runtime_launch::fixed_runtime_proxy_state(state, profile_name)
}

fn select_runtime_launch_profile(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<RuntimeLaunchSelection> {
    let mut selection = RuntimeLaunchSelection::resolve(
        paths,
        state,
        request.profile,
        request.model_provider_override,
    )?;
    if selection.non_openai_model_provider.is_some() {
        return Ok(selection);
    }
    if request.skip_quota_check {
        return Ok(selection);
    }

    if request.allow_auto_rotate
        && !selection.explicit_profile_requested
        && state.profiles.len() > 1
    {
        run_auto_runtime_launch_preflight(paths, state, request, &mut selection)?;
    } else {
        run_selected_runtime_launch_preflight(paths, state, request, &mut selection)?;
    }

    Ok(selection)
}

fn run_auto_runtime_launch_preflight(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
) -> Result<()> {
    let current_report = probe_run_profile(
        state,
        &selection.initial_profile_name,
        0,
        request.base_url,
        request.upstream_no_proxy,
    )?;
    if run_profile_probe_is_ready(&current_report, request.include_code_review) {
        return Ok(());
    }

    let persisted_usage_snapshots =
        load_runtime_usage_snapshots(paths, &state.profiles).unwrap_or_default();
    let reports = run_preflight_reports_with_current_first(
        state,
        &selection.initial_profile_name,
        current_report,
        request.base_url,
        request.upstream_no_proxy,
    );
    let ready_candidates = ready_profile_candidates(
        &reports,
        request.include_code_review,
        Some(&selection.initial_profile_name),
        state,
        Some(&persisted_usage_snapshots),
    );
    let selected_report = reports
        .iter()
        .find(|report| report.name == selection.initial_profile_name);

    if let Some(best_candidate) = ready_candidates.first() {
        if best_candidate.name != selection.initial_profile_name {
            rotate_to_scored_runtime_candidate(
                paths,
                state,
                request,
                selection,
                best_candidate,
                selected_report,
                request.include_code_review,
            )?;
        }
        return Ok(());
    }

    if let Some(report) = selected_report {
        handle_no_ready_runtime_profiles(report, &selection.initial_profile_name, request)?;
    }
    Ok(())
}

fn rotate_to_scored_runtime_candidate(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
    best_candidate: &ReadyProfileCandidate,
    selected_report: Option<&RunProfileProbeReport>,
    include_code_review: bool,
) -> Result<()> {
    print_wrapped_stderr(&section_header("Quota Preflight"));
    let selection_message = scored_runtime_candidate_message(
        &selection.initial_profile_name,
        best_candidate,
        selected_report,
        include_code_review,
    );

    activate_runtime_launch_profile(paths, state, request, selection, &best_candidate.name)?;
    if let Some(warning) = selection_message.warning.as_deref() {
        print_wrapped_stderr(warning);
    }
    print_wrapped_stderr(&selection_message.selection);
    Ok(())
}

fn scored_runtime_candidate_message(
    initial_profile_name: &str,
    best_candidate: &ReadyProfileCandidate,
    selected_report: Option<&RunProfileProbeReport>,
    include_code_review: bool,
) -> RuntimeLaunchScoredCandidateOutput {
    prodex_runtime_launch::scored_runtime_candidate_message(
        initial_profile_name,
        best_candidate,
        selected_report,
        include_code_review,
    )
}

fn handle_no_ready_runtime_profiles(
    report: &RunProfileProbeReport,
    profile_name: &str,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<()> {
    match prodex_runtime_launch::no_ready_runtime_profiles_plan(
        report,
        profile_name,
        request.include_code_review,
    ) {
        prodex_runtime_launch::RuntimeLaunchNoReadyProfilesPlan::Blocked {
            blocked_message,
            no_ready_message,
            inspect_hint,
            error_message,
        } => {
            print_wrapped_stderr(&section_header("Quota Preflight"));
            print_wrapped_stderr(&blocked_message);
            print_wrapped_stderr(&no_ready_message);
            print_wrapped_stderr(&inspect_hint);
            return Err(command_exit_error(2, error_message));
        }
        prodex_runtime_launch::RuntimeLaunchNoReadyProfilesPlan::ProbeFailed {
            warning_message,
            continue_message,
        } => {
            print_wrapped_stderr(&section_header("Quota Preflight"));
            print_wrapped_stderr(&warning_message);
            print_wrapped_stderr(&continue_message);
        }
    }
    Ok(())
}

fn run_selected_runtime_launch_preflight(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
) -> Result<()> {
    match fetch_usage_with_proxy_policy(
        &selection.codex_home,
        request.base_url,
        request.upstream_no_proxy,
    ) {
        Ok(usage) => {
            let blocked = collect_blocked_limits(&usage, request.include_code_review);
            if !blocked.is_empty() {
                handle_blocked_selected_runtime_profile(
                    paths, state, request, selection, &blocked,
                )?;
            }
        }
        Err(err) => {
            print_wrapped_stderr(&section_header("Quota Preflight"));
            print_wrapped_stderr(&format!(
                "Warning: quota preflight failed for '{}': {err:#}",
                selection.initial_profile_name
            ));
            print_wrapped_stderr("Continuing without quota gate.");
        }
    }

    Ok(())
}

fn handle_blocked_selected_runtime_profile(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
    blocked: &[BlockedLimit],
) -> Result<()> {
    let alternatives = find_ready_profiles(
        state,
        &selection.initial_profile_name,
        request.base_url,
        request.include_code_review,
        request.upstream_no_proxy,
    );
    let plan = prodex_runtime_launch::blocked_selected_runtime_profile_plan(
        &selection.initial_profile_name,
        blocked,
        request.allow_auto_rotate,
        &alternatives,
    );

    print_wrapped_stderr(&section_header("Quota Preflight"));
    match plan {
        prodex_runtime_launch::RuntimeLaunchBlockedSelectedProfilePlan::Rotate {
            blocked_message,
            next_profile,
            rotate_message,
        } => {
            print_wrapped_stderr(&blocked_message);
            activate_runtime_launch_profile(paths, state, request, selection, &next_profile)?;
            print_wrapped_stderr(&rotate_message);
        }
        prodex_runtime_launch::RuntimeLaunchBlockedSelectedProfilePlan::Stop {
            blocked_message,
            messages,
            error_message,
        } => {
            print_wrapped_stderr(&blocked_message);
            for message in messages {
                print_wrapped_stderr(&message);
            }
            return Err(command_exit_error(2, error_message));
        }
    }

    Ok(())
}

fn activate_runtime_launch_profile(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
    profile_name: &str,
) -> Result<()> {
    selection.select_profile(state, profile_name, request.model_provider_override)?;
    state.active_profile = Some(profile_name.to_string());
    state.save(paths)?;
    Ok(())
}

fn runtime_launch_profile_home(state: &AppState, profile_name: &str) -> Result<PathBuf> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    if !profile.provider.supports_codex_runtime() {
        bail!(
            "profile '{}' uses {}. `prodex run` currently supports OpenAI/Codex profiles only.",
            profile_name,
            profile.provider.display_name()
        );
    }
    Ok(profile.codex_home.clone())
}

#[cfg(test)]
#[path = "../../../../tests/unit/src/app_commands/runtime_launch.rs"]
mod tests;
