use super::*;

mod info;
mod runtime_launch;
mod shared;

pub(crate) use self::info::*;
use self::runtime_launch::*;
pub(crate) use self::shared::*;

pub(super) fn handle_info(_args: InfoArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let policy_summary = runtime_policy_summary()?;
    let runtime_metrics_targets = collect_runtime_broker_metrics_targets(&paths);
    let now = Local::now().timestamp();
    let version_summary = format_info_prodex_version(&paths)?;
    let quota = collect_info_quota_aggregate(&paths, &state, now);
    let processes = collect_prodex_processes();
    let runtime_logs = collect_active_runtime_log_paths(&processes);
    let runtime_load = collect_info_runtime_load_summary(&runtime_logs, now);
    let runtime_process_count = processes.iter().filter(|process| process.runtime).count();
    let five_hour_runway = estimate_info_runway(
        &runtime_load.observations,
        InfoQuotaWindow::FiveHour,
        quota.five_hour_pool_remaining,
        now,
    );
    let weekly_runway = estimate_info_runway(
        &runtime_load.observations,
        InfoQuotaWindow::Weekly,
        quota.weekly_pool_remaining,
        now,
    );

    let fields = vec![
        ("Profiles".to_string(), state.profiles.len().to_string()),
        (
            "Active profile".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
        (
            "Runtime policy".to_string(),
            format_runtime_policy_summary(policy_summary.as_ref()),
        ),
        (
            "Secret backend".to_string(),
            format_secret_backend_summary(),
        ),
        ("Runtime logs".to_string(), format_runtime_logs_summary()),
        ("Audit logs".to_string(), format_audit_logs_summary()),
        (
            "Runtime metrics".to_string(),
            format_runtime_broker_metrics_targets(&runtime_metrics_targets),
        ),
        ("Prodex version".to_string(), version_summary),
        (
            "Prodex processes".to_string(),
            format_info_process_summary(&processes),
        ),
        (
            "Recent load".to_string(),
            format_info_load_summary(&runtime_load, runtime_process_count),
        ),
        (
            "Quota data".to_string(),
            format_info_quota_data_summary(&quota),
        ),
        (
            "5h remaining pool".to_string(),
            format_info_pool_remaining(
                quota.five_hour_pool_remaining,
                quota.profiles_with_data(),
                quota.earliest_five_hour_reset_at,
            ),
        ),
        (
            "Weekly remaining pool".to_string(),
            format_info_pool_remaining(
                quota.weekly_pool_remaining,
                quota.profiles_with_data(),
                quota.earliest_weekly_reset_at,
            ),
        ),
        (
            "5h runway".to_string(),
            format_info_runway(
                quota.profiles_with_data(),
                quota.five_hour_pool_remaining,
                quota.earliest_five_hour_reset_at,
                five_hour_runway.as_ref(),
                now,
            ),
        ),
        (
            "Weekly runway".to_string(),
            format_info_runway(
                quota.profiles_with_data(),
                quota.weekly_pool_remaining,
                quota.earliest_weekly_reset_at,
                weekly_runway.as_ref(),
                now,
            ),
        ),
    ];
    print_panel("Info", &fields);
    Ok(())
}

pub(super) fn handle_doctor(args: DoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let codex_home = default_codex_home(&paths)?;
    let policy_summary = runtime_policy_summary()?;
    let runtime_metrics_targets = collect_runtime_broker_metrics_targets(&paths);

    if args.runtime && args.json {
        let summary = collect_runtime_doctor_summary();
        let mut value = runtime_doctor_json_value(&summary);
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "runtime_policy".to_string(),
                runtime_policy_json_value(policy_summary.as_ref()),
            );
            object.insert("secret_backend".to_string(), secret_backend_json_value());
            object.insert("runtime_logs".to_string(), runtime_logs_json_value());
            object.insert("audit_logs".to_string(), audit_logs_json_value());
            object.insert(
                "live_brokers".to_string(),
                serde_json::to_value(collect_live_runtime_broker_observations(&paths))
                    .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
            );
            object.insert(
                "live_broker_metrics_targets".to_string(),
                serde_json::to_value(&runtime_metrics_targets)
                    .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
            );
        }
        let json = serde_json::to_string_pretty(&value)
            .context("failed to serialize runtime doctor summary")?;
        println!("{json}");
        return Ok(());
    }

    let summary_fields = vec![
        ("Prodex root".to_string(), paths.root.display().to_string()),
        (
            "State file".to_string(),
            format!(
                "{} ({})",
                paths.state_file.display(),
                if paths.state_file.exists() {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Profiles root".to_string(),
            paths.managed_profiles_root.display().to_string(),
        ),
        (
            "Default CODEX_HOME".to_string(),
            format!(
                "{} ({})",
                codex_home.display(),
                if codex_home.exists() {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Codex binary".to_string(),
            format_binary_resolution(&codex_bin()),
        ),
        (
            "Quota endpoint".to_string(),
            usage_url(&quota_base_url(None)),
        ),
        (
            "Runtime policy".to_string(),
            format_runtime_policy_summary(policy_summary.as_ref()),
        ),
        (
            "Secret backend".to_string(),
            format_secret_backend_summary(),
        ),
        ("Runtime logs".to_string(), format_runtime_logs_summary()),
        ("Audit logs".to_string(), format_audit_logs_summary()),
        (
            "Runtime metrics".to_string(),
            format_runtime_broker_metrics_targets(&runtime_metrics_targets),
        ),
        ("Profiles".to_string(), state.profiles.len().to_string()),
        (
            "Active profile".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
    ];
    print_panel("Doctor", &summary_fields);

    if args.runtime {
        println!();
        print_panel("Runtime Proxy", &runtime_doctor_fields());
    }

    if state.profiles.is_empty() {
        return Ok(());
    }

    for report in collect_doctor_profile_reports(&state, args.quota) {
        let kind = if report.summary.managed {
            "managed"
        } else {
            "external"
        };

        println!();
        let mut fields = vec![
            (
                "Current".to_string(),
                if report.summary.active {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
            ("Kind".to_string(), kind.to_string()),
            ("Auth".to_string(), report.summary.auth.label),
            (
                "Email".to_string(),
                report.summary.email.as_deref().unwrap_or("-").to_string(),
            ),
            (
                "Path".to_string(),
                report.summary.codex_home.display().to_string(),
            ),
            (
                "Exists".to_string(),
                if report.summary.codex_home.exists() {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
        ];

        if let Some(quota) = report.quota {
            match quota {
                Ok(usage) => {
                    let blocked = collect_blocked_limits(&usage, false);
                    fields.push((
                        "Quota".to_string(),
                        if blocked.is_empty() {
                            "Ready".to_string()
                        } else {
                            format!("Blocked ({})", format_blocked_limits(&blocked))
                        },
                    ));
                    fields.push(("Main".to_string(), format_main_windows(&usage)));
                }
                Err(err) => {
                    fields.push((
                        "Quota".to_string(),
                        format!("Error ({})", first_line_of_error(&err.to_string())),
                    ));
                }
            }
        }
        print_panel(&format!("Profile {}", report.summary.name), &fields);
    }

    Ok(())
}

pub(super) fn handle_audit(args: AuditArgs) -> Result<()> {
    let query = AuditLogQuery {
        tail: args.tail,
        component: args.component.as_deref().map(str::trim).map(str::to_string),
        action: args.action.as_deref().map(str::trim).map(str::to_string),
        outcome: args.outcome.as_deref().map(str::trim).map(str::to_string),
    };
    let events = read_recent_audit_events(&query)?;

    if args.json {
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "audit_logs": audit_logs_json_value(),
            "filters": {
                "tail": query.tail,
                "component": query.component,
                "action": query.action,
                "outcome": query.outcome,
            },
            "events": events,
        }))
        .context("failed to serialize audit log output")?;
        println!("{json}");
        return Ok(());
    }

    println!(
        "{}",
        render_audit_events_human(&audit_log_path(), &query, &events)
    );

    Ok(())
}

pub(super) fn handle_cleanup() -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let runtime_log_dir = runtime_proxy_log_dir();
    let summary = perform_prodex_cleanup(&paths, &mut state)?;

    let fields = vec![
        ("Prodex root".to_string(), paths.root.display().to_string()),
        (
            "Duplicate profiles".to_string(),
            summary.duplicate_profiles_removed.to_string(),
        ),
        (
            "Duplicate managed homes".to_string(),
            summary.duplicate_managed_profile_homes_removed.to_string(),
        ),
        (
            "Runtime logs".to_string(),
            format!(
                "{} removed from {}",
                summary.runtime_logs_removed,
                runtime_log_dir.display()
            ),
        ),
        (
            "Runtime pointer".to_string(),
            if summary.stale_runtime_log_pointer_removed > 0 {
                "removed stale latest-pointer file".to_string()
            } else {
                "clean".to_string()
            },
        ),
        (
            "Temp login homes".to_string(),
            summary.stale_login_dirs_removed.to_string(),
        ),
        (
            "Orphan managed homes".to_string(),
            summary.orphan_managed_profile_dirs_removed.to_string(),
        ),
        (
            "Transient root files".to_string(),
            summary.transient_root_files_removed.to_string(),
        ),
        (
            "Stale root temp files".to_string(),
            summary.stale_root_temp_files_removed.to_string(),
        ),
        (
            "Dead broker leases".to_string(),
            summary.dead_runtime_broker_leases_removed.to_string(),
        ),
        (
            "Dead broker registries".to_string(),
            summary.dead_runtime_broker_registries_removed.to_string(),
        ),
        (
            "Total removed".to_string(),
            summary.total_removed().to_string(),
        ),
    ];
    print_panel("Cleanup", &fields);
    Ok(())
}

pub(super) fn deserialize_null_default<'de, D, T>(
    deserializer: D,
) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

pub(super) fn handle_quota(args: QuotaArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    if args.all {
        if state.profiles.is_empty() {
            bail!("no profiles configured");
        }
        if quota_watch_enabled(&args) {
            return watch_all_quotas(&paths, args.base_url.as_deref(), args.detail);
        }
        let reports = collect_quota_reports(&state, args.base_url.as_deref());
        print_quota_reports(&reports, args.detail);
        return Ok(());
    }

    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    if args.raw {
        let usage = fetch_usage_json(&codex_home, args.base_url.as_deref())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&usage).context("failed to render usage JSON")?
        );
        return Ok(());
    }

    if quota_watch_enabled(&args) {
        return watch_quota(&profile_name, &codex_home, args.base_url.as_deref());
    }

    let usage = fetch_usage(&codex_home, args.base_url.as_deref())?;
    println!("{}", render_profile_quota(&profile_name, &usage));
    Ok(())
}

struct RunCommandStrategy {
    args: RunArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
}

impl RunCommandStrategy {
    fn new(args: RunArgs) -> Self {
        let codex_args = normalize_run_codex_args(&args.codex_args);
        let include_code_review = is_review_invocation(&codex_args);
        Self {
            args,
            codex_args,
            include_code_review,
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
        }
    }

    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &self.codex_args);
        Ok(RuntimeLaunchPlan::new(
            ChildProcessPlan::new(codex_bin(), prepared.codex_home.clone()).with_args(runtime_args),
        ))
    }
}

pub(super) fn handle_run(args: RunArgs) -> Result<()> {
    execute_runtime_launch(RunCommandStrategy::new(args))
}

#[derive(Debug, Clone)]
struct RuntimeLaunchSelection {
    initial_profile_name: String,
    selected_profile_name: String,
    codex_home: PathBuf,
    explicit_profile_requested: bool,
}

impl RuntimeLaunchSelection {
    fn resolve(state: &AppState, requested: Option<&str>) -> Result<Self> {
        let profile_name = resolve_profile_name(state, requested)?;
        let codex_home = runtime_launch_profile_home(state, &profile_name)?;

        Ok(Self {
            initial_profile_name: profile_name.clone(),
            selected_profile_name: profile_name,
            codex_home,
            explicit_profile_requested: requested.is_some(),
        })
    }

    fn select_profile(&mut self, state: &AppState, profile_name: &str) -> Result<()> {
        self.codex_home = runtime_launch_profile_home(state, profile_name)?;
        self.selected_profile_name = profile_name.to_string();
        Ok(())
    }
}

pub(super) fn prepare_runtime_launch(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let selection = select_runtime_launch_profile(&paths, &mut state, &request)?;

    record_run_selection(&mut state, &selection.selected_profile_name);
    state.save(&paths)?;

    let managed = state
        .profiles
        .get(&selection.selected_profile_name)
        .with_context(|| format!("profile '{}' is missing", selection.selected_profile_name))?
        .managed;
    if managed {
        prepare_managed_codex_home(&paths, &selection.codex_home)?;
    }

    let runtime_upstream_base_url = quota_base_url(request.base_url);
    let runtime_proxy = if request.force_runtime_proxy
        || should_enable_runtime_rotation_proxy(
            &state,
            &selection.selected_profile_name,
            request.allow_auto_rotate,
        ) {
        Some(ensure_runtime_rotation_proxy_endpoint(
            &paths,
            &selection.selected_profile_name,
            runtime_upstream_base_url.as_str(),
            request.include_code_review,
        )?)
    } else {
        None
    };

    Ok(PreparedRuntimeLaunch {
        paths,
        codex_home: selection.codex_home,
        managed,
        runtime_proxy,
    })
}

pub(super) fn handle_runtime_broker(args: RuntimeBrokerArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let proxy = start_runtime_rotation_proxy_with_listen_addr(
        &paths,
        &state,
        &args.current_profile,
        args.upstream_base_url.clone(),
        args.include_code_review,
        args.listen_addr.as_deref(),
    )?;
    if proxy.owner_lock.is_none() {
        return Ok(());
    }

    let metadata = RuntimeBrokerMetadata {
        broker_key: runtime_broker_key(&args.upstream_base_url, args.include_code_review),
        listen_addr: proxy.listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        current_profile: args.current_profile.clone(),
        include_code_review: args.include_code_review,
        instance_token: args.instance_token.clone(),
        admin_token: args.admin_token.clone(),
    };
    register_runtime_broker_metadata(&proxy.log_path, metadata.clone());
    let registry = RuntimeBrokerRegistry {
        pid: std::process::id(),
        listen_addr: proxy.listen_addr.to_string(),
        started_at: metadata.started_at,
        upstream_base_url: args.upstream_base_url.clone(),
        include_code_review: args.include_code_review,
        current_profile: args.current_profile.clone(),
        instance_token: args.instance_token.clone(),
        admin_token: args.admin_token.clone(),
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
    };
    save_runtime_broker_registry(&paths, &args.broker_key, &registry)?;
    runtime_proxy_log_to_path(
        &proxy.log_path,
        &format!(
            "runtime_broker_started listen_addr={} broker_key={} current_profile={} include_code_review={}",
            proxy.listen_addr, args.broker_key, args.current_profile, args.include_code_review
        ),
    );
    audit_log_event_best_effort(
        "runtime_broker",
        "start",
        "success",
        serde_json::json!({
            "broker_key": args.broker_key,
            "listen_addr": proxy.listen_addr.to_string(),
            "current_profile": args.current_profile,
            "include_code_review": args.include_code_review,
            "upstream_base_url": args.upstream_base_url,
        }),
    );

    let startup_grace_until = metadata
        .started_at
        .saturating_add(runtime_broker_startup_grace_seconds());
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    let lease_scan_interval = Duration::from_millis(
        RUNTIME_BROKER_LEASE_SCAN_INTERVAL_MS.max(RUNTIME_BROKER_POLL_INTERVAL_MS),
    );
    let mut idle_started_at = None::<i64>;
    let mut cached_live_leases = 0usize;
    let mut last_lease_scan_at = Instant::now() - lease_scan_interval;
    loop {
        let active_requests = proxy.active_request_count.load(Ordering::SeqCst);
        if active_requests == 0 && last_lease_scan_at.elapsed() >= lease_scan_interval {
            cached_live_leases = cleanup_runtime_broker_stale_leases(&paths, &args.broker_key);
            last_lease_scan_at = Instant::now();
        }
        if cached_live_leases > 0 || active_requests > 0 {
            idle_started_at = None;
        } else {
            let now = Local::now().timestamp();
            if now < startup_grace_until {
                idle_started_at = None;
                thread::sleep(poll_interval);
                continue;
            }
            let idle_since = idle_started_at.get_or_insert(now);
            if now.saturating_sub(*idle_since) >= RUNTIME_BROKER_IDLE_GRACE_SECONDS {
                runtime_proxy_log_to_path(
                    &proxy.log_path,
                    &format!(
                        "runtime_broker_idle_shutdown broker_key={} idle_seconds={}",
                        args.broker_key,
                        now.saturating_sub(*idle_since)
                    ),
                );
                break;
            }
        }
        thread::sleep(poll_interval);
    }

    drop(proxy);
    remove_runtime_broker_registry_if_token_matches(&paths, &args.broker_key, &args.instance_token);
    Ok(())
}

pub(super) fn run_child_plan(
    plan: &ChildProcessPlan,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Result<ExitStatus> {
    let mut command = Command::new(&plan.binary);
    command.args(&plan.args).env("CODEX_HOME", &plan.codex_home);
    for key in &plan.removed_env {
        command.env_remove(key);
    }
    for (key, value) in &plan.extra_env {
        command.env(key, value);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to execute {}", plan.binary.to_string_lossy()))?;
    let _child_runtime_broker_lease = match runtime_proxy {
        Some(proxy) => match proxy.create_child_lease(child.id()) {
            Ok(lease) => Some(lease),
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(err);
            }
        },
        None => None,
    };
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for {}", plan.binary.to_string_lossy()))?;
    Ok(status)
}

pub(super) fn exit_with_status(status: ExitStatus) -> Result<()> {
    std::process::exit(status.code().unwrap_or(1));
}

pub(super) fn collect_run_profile_reports(
    state: &AppState,
    profile_names: Vec<String>,
    base_url: Option<&str>,
) -> Vec<RunProfileProbeReport> {
    let jobs = profile_names
        .into_iter()
        .enumerate()
        .filter_map(|(order_index, name)| {
            let profile = state.profiles.get(&name)?;
            Some(RunProfileProbeJob {
                name,
                order_index,
                codex_home: profile.codex_home.clone(),
            })
        })
        .collect();
    let base_url = base_url.map(str::to_owned);

    map_parallel(jobs, |job| {
        let auth = read_auth_summary(&job.codex_home);
        let result = if auth.quota_compatible {
            fetch_usage(&job.codex_home, base_url.as_deref()).map_err(|err| err.to_string())
        } else {
            Err("auth mode is not quota-compatible".to_string())
        };

        RunProfileProbeReport {
            name: job.name,
            order_index: job.order_index,
            auth,
            result,
        }
    })
}

pub(super) fn probe_run_profile(
    state: &AppState,
    profile_name: &str,
    order_index: usize,
    base_url: Option<&str>,
) -> Result<RunProfileProbeReport> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let auth = read_auth_summary(&profile.codex_home);
    let result = if auth.quota_compatible {
        fetch_usage(&profile.codex_home, base_url).map_err(|err| err.to_string())
    } else {
        Err("auth mode is not quota-compatible".to_string())
    };

    Ok(RunProfileProbeReport {
        name: profile_name.to_string(),
        order_index,
        auth,
        result,
    })
}

pub(super) fn run_profile_probe_is_ready(
    report: &RunProfileProbeReport,
    include_code_review: bool,
) -> bool {
    match report.result.as_ref() {
        Ok(usage) => collect_blocked_limits(usage, include_code_review).is_empty(),
        Err(_) => false,
    }
}

pub(super) fn run_preflight_reports_with_current_first(
    state: &AppState,
    current_profile: &str,
    current_report: RunProfileProbeReport,
    base_url: Option<&str>,
) -> Vec<RunProfileProbeReport> {
    let mut reports = Vec::with_capacity(state.profiles.len());
    reports.push(current_report);
    reports.extend(
        collect_run_profile_reports(
            state,
            profile_rotation_order(state, current_profile),
            base_url,
        )
        .into_iter()
        .map(|mut report| {
            report.order_index += 1;
            report
        }),
    );
    reports
}

pub(super) fn ready_profile_candidates(
    reports: &[RunProfileProbeReport],
    include_code_review: bool,
    preferred_profile: Option<&str>,
    state: &AppState,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
) -> Vec<ReadyProfileCandidate> {
    let candidates = reports
        .iter()
        .filter_map(|report| {
            if !report.auth.quota_compatible {
                return None;
            }

            let (usage, quota_source) = match report.result.as_ref() {
                Ok(usage) => (usage.clone(), RuntimeQuotaSource::LiveProbe),
                Err(_) => {
                    let snapshot = persisted_usage_snapshots
                        .and_then(|snapshots| snapshots.get(&report.name))?;
                    let now = Local::now().timestamp();
                    if !runtime_usage_snapshot_is_usable(snapshot, now) {
                        return None;
                    }
                    (
                        usage_from_runtime_usage_snapshot(snapshot),
                        RuntimeQuotaSource::PersistedSnapshot,
                    )
                }
            };
            if !collect_blocked_limits(&usage, include_code_review).is_empty() {
                return None;
            }

            Some(ReadyProfileCandidate {
                name: report.name.clone(),
                usage,
                order_index: report.order_index,
                preferred: preferred_profile == Some(report.name.as_str()),
                quota_source,
            })
        })
        .collect::<Vec<_>>();

    schedule_ready_profile_candidates(candidates, state, preferred_profile)
}

pub(super) fn schedule_ready_profile_candidates(
    mut candidates: Vec<ReadyProfileCandidate>,
    state: &AppState,
    preferred_profile: Option<&str>,
) -> Vec<ReadyProfileCandidate> {
    if candidates.len() <= 1 {
        return candidates;
    }

    let now = Local::now().timestamp();
    let best_total_pressure = candidates
        .iter()
        .map(|candidate| ready_profile_score(candidate).total_pressure)
        .min()
        .unwrap_or(i64::MAX);

    candidates.sort_by_key(|candidate| {
        ready_profile_runtime_sort_key(candidate, state, best_total_pressure, now)
    });

    if let Some(preferred_name) = preferred_profile
        && let Some(preferred_index) = candidates.iter().position(|candidate| {
            candidate.name == preferred_name
                && !profile_in_run_selection_cooldown(state, &candidate.name, now)
        })
    {
        let preferred_score = ready_profile_score(&candidates[preferred_index]).total_pressure;
        let selected_score = ready_profile_score(&candidates[0]).total_pressure;

        if preferred_index > 0
            && score_within_bps(
                preferred_score,
                selected_score,
                RUN_SELECTION_HYSTERESIS_BPS,
            )
        {
            let preferred_candidate = candidates.remove(preferred_index);
            candidates.insert(0, preferred_candidate);
        }
    }

    candidates
}

pub(super) type ReadyProfileSortKey = (
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
    usize,
    usize,
    usize,
);

pub(super) type ReadyProfileRuntimeSortKey = (usize, usize, i64, ReadyProfileSortKey);

pub(super) fn ready_profile_runtime_sort_key(
    candidate: &ReadyProfileCandidate,
    state: &AppState,
    best_total_pressure: i64,
    now: i64,
) -> ReadyProfileRuntimeSortKey {
    let score = ready_profile_score(candidate);
    let near_optimal = score_within_bps(
        score.total_pressure,
        best_total_pressure,
        RUN_SELECTION_NEAR_OPTIMAL_BPS,
    );
    let recently_used =
        near_optimal && profile_in_run_selection_cooldown(state, &candidate.name, now);
    let last_selected_at = if near_optimal {
        state
            .last_run_selected_at
            .get(&candidate.name)
            .copied()
            .unwrap_or(i64::MIN)
    } else {
        i64::MIN
    };

    (
        if near_optimal { 0usize } else { 1usize },
        if recently_used { 1usize } else { 0usize },
        last_selected_at,
        ready_profile_sort_key(candidate),
    )
}

pub(super) fn ready_profile_sort_key(candidate: &ReadyProfileCandidate) -> ReadyProfileSortKey {
    let score = ready_profile_score(candidate);

    (
        score.total_pressure,
        score.weekly_pressure,
        score.five_hour_pressure,
        Reverse(score.reserve_floor),
        Reverse(score.weekly_remaining),
        Reverse(score.five_hour_remaining),
        score.weekly_reset_at,
        score.five_hour_reset_at,
        runtime_quota_source_sort_key(RuntimeRouteKind::Responses, candidate.quota_source),
        if candidate.preferred { 0usize } else { 1usize },
        candidate.order_index,
    )
}

pub(super) fn ready_profile_score(candidate: &ReadyProfileCandidate) -> ReadyProfileScore {
    ready_profile_score_for_route(&candidate.usage, RuntimeRouteKind::Responses)
}

pub(super) fn ready_profile_score_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> ReadyProfileScore {
    let weekly = required_main_window_snapshot(usage, "weekly");
    let five_hour = required_main_window_snapshot(usage, "5h");

    let weekly_pressure = weekly.map_or(i64::MAX, |window| window.pressure_score);
    let five_hour_pressure = five_hour.map_or(i64::MAX, |window| window.pressure_score);
    let weekly_remaining = weekly.map_or(0, |window| window.remaining_percent);
    let five_hour_remaining = five_hour.map_or(0, |window| window.remaining_percent);
    let weekly_weight = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => 10,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 8,
    };
    let reserve_bias = match runtime_quota_pressure_band_for_route(usage, route_kind) {
        RuntimeQuotaPressureBand::Healthy => 0,
        RuntimeQuotaPressureBand::Thin => 250_000,
        RuntimeQuotaPressureBand::Critical => 1_000_000,
        RuntimeQuotaPressureBand::Exhausted | RuntimeQuotaPressureBand::Unknown => i64::MAX / 4,
    };

    ReadyProfileScore {
        total_pressure: reserve_bias
            .saturating_add(weekly_pressure.saturating_mul(weekly_weight))
            .saturating_add(five_hour_pressure),
        weekly_pressure,
        five_hour_pressure,
        reserve_floor: weekly_remaining.min(five_hour_remaining),
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at: weekly.map_or(i64::MAX, |window| window.reset_at),
        five_hour_reset_at: five_hour.map_or(i64::MAX, |window| window.reset_at),
    }
}

pub(super) fn profile_in_run_selection_cooldown(
    state: &AppState,
    profile_name: &str,
    now: i64,
) -> bool {
    let Some(last_selected_at) = state.last_run_selected_at.get(profile_name).copied() else {
        return false;
    };

    now.saturating_sub(last_selected_at) < RUN_SELECTION_COOLDOWN_SECONDS
}

pub(super) fn score_within_bps(candidate_score: i64, best_score: i64, bps: i64) -> bool {
    if candidate_score <= best_score {
        return true;
    }

    let lhs = i128::from(candidate_score).saturating_mul(10_000);
    let rhs = i128::from(best_score).saturating_mul(i128::from(10_000 + bps));
    lhs <= rhs
}

pub(super) fn required_main_window_snapshot(
    usage: &UsageResponse,
    label: &str,
) -> Option<MainWindowSnapshot> {
    let window = find_main_window(usage.rate_limit.as_ref()?, label)?;
    let remaining_percent = remaining_percent(window.used_percent);
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let seconds_until_reset = if reset_at == i64::MAX {
        i64::MAX
    } else {
        (reset_at - Local::now().timestamp()).max(0)
    };
    let pressure_score = seconds_until_reset
        .saturating_mul(1_000)
        .checked_div(remaining_percent.max(1))
        .unwrap_or(i64::MAX);

    Some(MainWindowSnapshot {
        remaining_percent,
        reset_at,
        pressure_score,
    })
}

pub(super) fn active_profile_selection_order(
    state: &AppState,
    current_profile: &str,
) -> Vec<String> {
    std::iter::once(current_profile.to_string())
        .chain(profile_rotation_order(state, current_profile))
        .collect()
}

pub(super) fn map_parallel<I, O, F>(inputs: Vec<I>, func: F) -> Vec<O>
where
    I: Send,
    O: Send,
    F: Fn(I) -> O + Sync,
{
    if inputs.len() <= 1 {
        return inputs.into_iter().map(func).collect();
    }

    thread::scope(|scope| {
        let func = &func;
        let mut handles = Vec::with_capacity(inputs.len());
        for input in inputs {
            handles.push(scope.spawn(move || func(input)));
        }

        handles
            .into_iter()
            .map(|handle| handle.join().expect("parallel worker panicked"))
            .collect()
    })
}
pub(super) fn find_ready_profiles(
    state: &AppState,
    current_profile: &str,
    base_url: Option<&str>,
    include_code_review: bool,
) -> Vec<String> {
    ready_profile_candidates(
        &collect_run_profile_reports(
            state,
            profile_rotation_order(state, current_profile),
            base_url,
        ),
        include_code_review,
        None,
        state,
        None,
    )
    .into_iter()
    .map(|candidate| candidate.name)
    .collect()
}

pub(super) fn profile_rotation_order(state: &AppState, current_profile: &str) -> Vec<String> {
    let names: Vec<String> = state.profiles.keys().cloned().collect();
    let Some(index) = names.iter().position(|name| name == current_profile) else {
        return names
            .into_iter()
            .filter(|name| name != current_profile)
            .collect();
    };

    names
        .iter()
        .skip(index + 1)
        .chain(names.iter().take(index))
        .cloned()
        .collect()
}

pub(super) fn is_review_invocation(args: &[OsString]) -> bool {
    args.iter().any(|arg| arg == "review")
}
