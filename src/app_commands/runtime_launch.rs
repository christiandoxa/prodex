use super::*;

struct RunCommandStrategy {
    args: RunArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
    mem_mode: bool,
    dry_run: bool,
    model_provider_override: Option<String>,
}

impl RunCommandStrategy {
    fn new(args: RunArgs) -> Self {
        let (mem_mode, codex_args) = runtime_mem_extract_mode(&args.codex_args);
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
        if self.mem_mode {
            ensure_runtime_mem_prodex_observer(&prepared.paths)?;
            ensure_runtime_mem_codex_watch_for_home(&prepared.codex_home)?;
        }
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &self.codex_args);
        Ok(RuntimeLaunchPlan::new(codex_child_plan(
            prepared.codex_home.clone(),
            runtime_args,
        )))
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
}

impl RuntimeLaunchSelection {
    fn resolve(
        state: &AppState,
        requested: Option<&str>,
        model_provider_override: Option<&str>,
    ) -> Result<Self> {
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
        print_wrapped_stderr(&format!(
            "Detected model_provider '{}' from {}. Launching directly without prodex quota preflight or auto-rotate proxy.",
            setting.provider_id,
            setting.source.display_name(),
        ));
        Ok(())
    }

    fn record_selection(&mut self) -> Result<()> {
        record_run_selection(&mut self.state, &self.selection.selected_profile_name);
        self.state.save(&self.paths)?;
        Ok(())
    }

    fn selected_profile_is_managed(&self) -> Result<bool> {
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
    let selection =
        RuntimeLaunchSelection::resolve(&state, request.profile, request.model_provider_override)?;
    let managed = state
        .profiles
        .get(&selection.selected_profile_name)
        .with_context(|| format!("profile '{}' is missing", selection.selected_profile_name))?
        .managed;
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
    })
}

fn select_runtime_launch_profile(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<RuntimeLaunchSelection> {
    let mut selection =
        RuntimeLaunchSelection::resolve(state, request.profile, request.model_provider_override)?;
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
    let current_report =
        probe_run_profile(state, &selection.initial_profile_name, 0, request.base_url)?;
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
        handle_no_ready_runtime_profiles(report, &selection.initial_profile_name, request);
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
    print_wrapped_stderr(&selection_message);
    Ok(())
}

fn scored_runtime_candidate_message(
    initial_profile_name: &str,
    best_candidate: &ReadyProfileCandidate,
    selected_report: Option<&RunProfileProbeReport>,
    include_code_review: bool,
) -> String {
    let mut selection_message = format!(
        "Using profile '{}' ({})",
        best_candidate.name,
        format_main_windows_compact(&best_candidate.usage)
    );

    if let Some(report) = selected_report {
        match &report.result {
            Ok(usage) => {
                let blocked = collect_blocked_limits(usage, include_code_review);
                if !blocked.is_empty() {
                    print_wrapped_stderr(&format!(
                        "Quota preflight blocked profile '{}': {}",
                        initial_profile_name,
                        format_blocked_limits(&blocked)
                    ));
                    selection_message = format!(
                        "Auto-rotating to profile '{}' using quota-pressure scoring ({}).",
                        best_candidate.name,
                        format_main_windows_compact(&best_candidate.usage)
                    );
                } else {
                    selection_message = format!(
                        "Auto-selecting profile '{}' over active profile '{}' using quota-pressure scoring ({}).",
                        best_candidate.name,
                        initial_profile_name,
                        format_main_windows_compact(&best_candidate.usage)
                    );
                }
            }
            Err(err) => {
                print_wrapped_stderr(&format!(
                    "Warning: quota preflight failed for '{}': {err}",
                    initial_profile_name
                ));
                selection_message = format!(
                    "Using ready profile '{}' after quota preflight failed ({})",
                    best_candidate.name,
                    format_main_windows_compact(&best_candidate.usage)
                );
            }
        }
    }

    selection_message
}

fn handle_no_ready_runtime_profiles(
    report: &RunProfileProbeReport,
    profile_name: &str,
    request: &RuntimeLaunchRequest<'_>,
) {
    match &report.result {
        Ok(usage) => {
            let blocked = collect_blocked_limits(usage, request.include_code_review);
            print_wrapped_stderr(&section_header("Quota Preflight"));
            print_wrapped_stderr(&format!(
                "Quota preflight blocked profile '{}': {}",
                profile_name,
                format_blocked_limits(&blocked)
            ));
            print_wrapped_stderr("No ready profile was found.");
            print_quota_preflight_inspect_hint(profile_name);
            std::process::exit(2);
        }
        Err(err) => {
            print_wrapped_stderr(&section_header("Quota Preflight"));
            print_wrapped_stderr(&format!(
                "Warning: quota preflight failed for '{}': {err:#}",
                profile_name
            ));
            print_wrapped_stderr("Continuing without quota gate.");
        }
    }
}

fn run_selected_runtime_launch_preflight(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
) -> Result<()> {
    match fetch_usage(&selection.codex_home, request.base_url) {
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
    );

    print_wrapped_stderr(&section_header("Quota Preflight"));
    print_wrapped_stderr(&format!(
        "Quota preflight blocked profile '{}': {}",
        selection.initial_profile_name,
        format_blocked_limits(blocked)
    ));

    if request.allow_auto_rotate {
        if let Some(next_profile) = alternatives.first() {
            let next_profile = next_profile.clone();
            activate_runtime_launch_profile(paths, state, request, selection, &next_profile)?;
            print_wrapped_stderr(&format!("Auto-rotating to profile '{}'.", next_profile));
        } else {
            print_wrapped_stderr("No other ready profile was found.");
            print_quota_preflight_inspect_hint(&selection.initial_profile_name);
            std::process::exit(2);
        }
    } else {
        if !alternatives.is_empty() {
            print_wrapped_stderr(&format!(
                "Other profiles that look ready: {}",
                alternatives.join(", ")
            ));
            print_wrapped_stderr("Rerun without `--no-auto-rotate` to allow fallback.");
        }
        print_quota_preflight_inspect_hint(&selection.initial_profile_name);
        std::process::exit(2);
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

fn print_quota_preflight_inspect_hint(profile_name: &str) {
    print_wrapped_stderr(&format!(
        "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
        profile_name
    ));
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
mod tests {
    use super::*;

    #[test]
    fn prepare_runtime_launch_skips_proxy_for_non_openai_model_provider() {
        let root = temp_dir("skip-proxy-non-openai");
        let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
        let bedrock_home = root.join("bedrock-home");
        let openai_home = root.join("openai-home");
        fs::create_dir_all(&bedrock_home).unwrap();
        fs::create_dir_all(&openai_home).unwrap();
        fs::write(
            bedrock_home.join("config.toml"),
            "model_provider = 'amazon-bedrock'\n",
        )
        .unwrap();
        fs::write(
            secret_store::auth_json_path(&openai_home),
            r#"{"tokens":{"access_token":"chatgpt-token"}}"#,
        )
        .unwrap();
        write_state(
            &root,
            AppState {
                active_profile: Some("bedrock".to_string()),
                profiles: BTreeMap::from([
                    (
                        "bedrock".to_string(),
                        ProfileEntry {
                            codex_home: bedrock_home.clone(),
                            managed: false,
                            email: None,
                            provider: ProfileProvider::Openai,
                        },
                    ),
                    (
                        "openai".to_string(),
                        ProfileEntry {
                            codex_home: openai_home,
                            managed: false,
                            email: None,
                            provider: ProfileProvider::Openai,
                        },
                    ),
                ]),
                ..AppState::default()
            },
        );

        let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
            profile: Some("bedrock"),
            allow_auto_rotate: true,
            skip_quota_check: false,
            base_url: None,
            include_code_review: false,
            force_runtime_proxy: false,
            model_provider_override: None,
        })
        .unwrap();

        assert_eq!(prepared.codex_home, bedrock_home);
        assert!(prepared.runtime_proxy.is_none());
    }

    #[test]
    fn prepare_runtime_launch_rejects_claude_for_non_openai_model_provider() {
        let root = temp_dir("reject-claude-non-openai");
        let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
        let bedrock_home = root.join("bedrock-home");
        fs::create_dir_all(&bedrock_home).unwrap();
        fs::write(
            bedrock_home.join("config.toml"),
            "model_provider = 'amazon-bedrock'\n",
        )
        .unwrap();
        write_state(
            &root,
            AppState {
                active_profile: Some("bedrock".to_string()),
                profiles: BTreeMap::from([(
                    "bedrock".to_string(),
                    ProfileEntry {
                        codex_home: bedrock_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                )]),
                ..AppState::default()
            },
        );

        let err = match prepare_runtime_launch(RuntimeLaunchRequest {
            profile: Some("bedrock"),
            allow_auto_rotate: true,
            skip_quota_check: false,
            base_url: None,
            include_code_review: false,
            force_runtime_proxy: true,
            model_provider_override: None,
        }) {
            Ok(_) => panic!("expected Claude launch to reject non-OpenAI model providers"),
            Err(err) => err,
        };

        let message = format!("{err:#}");
        assert!(message.contains("amazon-bedrock"));
        assert!(message.contains("prodex claude"));
    }

    #[test]
    fn prepare_runtime_launch_dry_run_uses_proxy_preview_without_recording_selection() {
        let root = temp_dir("dry-run-preview-no-selection-save");
        let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
        let main_home = root.join("main-home");
        let second_home = root.join("second-home");
        fs::create_dir_all(&main_home).unwrap();
        fs::create_dir_all(&second_home).unwrap();
        fs::write(
            secret_store::auth_json_path(&main_home),
            r#"{"tokens":{"access_token":"main-token"}}"#,
        )
        .unwrap();
        fs::write(
            secret_store::auth_json_path(&second_home),
            r#"{"tokens":{"access_token":"second-token"}}"#,
        )
        .unwrap();
        write_state(
            &root,
            AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([
                    (
                        "main".to_string(),
                        ProfileEntry {
                            codex_home: main_home.clone(),
                            managed: false,
                            email: None,
                            provider: ProfileProvider::Openai,
                        },
                    ),
                    (
                        "second".to_string(),
                        ProfileEntry {
                            codex_home: second_home,
                            managed: false,
                            email: None,
                            provider: ProfileProvider::Openai,
                        },
                    ),
                ]),
                ..AppState::default()
            },
        );

        let prepared = prepare_runtime_launch_dry_run(RuntimeLaunchRequest {
            profile: None,
            allow_auto_rotate: true,
            skip_quota_check: false,
            base_url: None,
            include_code_review: false,
            force_runtime_proxy: false,
            model_provider_override: None,
        })
        .unwrap();

        assert_eq!(prepared.codex_home, main_home);
        assert_eq!(
            prepared
                .runtime_proxy
                .as_ref()
                .expect("runtime proxy preview should exist")
                .listen_addr
                .port(),
            0
        );
        let paths = AppPaths::discover().unwrap();
        let state = AppState::load(&paths).unwrap();
        assert!(
            state.last_run_selected_at.is_empty(),
            "dry-run must not record launch selection"
        );
    }

    fn write_state(root: &Path, state: AppState) {
        fs::create_dir_all(root).unwrap();
        let paths = AppPaths::discover().unwrap();
        state.save(&paths).unwrap();
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "prodex-runtime-launch-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        dir
    }
}
