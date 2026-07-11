use super::super::*;

pub(super) struct RuntimeResponseSelectionPrepared {
    pub(super) pressure_mode: bool,
    pub(super) sync_probe_pressure_mode: bool,
    pub(super) inflight_soft_limit: usize,
    pub(super) selection_state: RuntimeRouteSelectionCatalog,
    pub(super) ready_candidates: Vec<ReadyProfileCandidate>,
    pub(super) report_count: usize,
    probe_counts: RuntimeResponseProbeCounts,
}

#[derive(Clone, Copy)]
struct RuntimeResponseProbeCounts {
    stale_refreshes: usize,
    cold_start_jobs: usize,
    sync_probe_jobs: usize,
    sync_probe_inline: bool,
}

pub(super) fn prepare_runtime_response_selection(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<RuntimeResponseSelectionPrepared> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let sync_probe_pressure_mode =
        runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit =
        runtime_profile_inflight_soft_limit_for_shared(shared, route_kind, pressure_mode);
    let selection_state = load_runtime_route_selection_catalog(shared, route_kind, now)?;
    let probe_plan = build_runtime_response_probe_plan(
        &selection_state,
        excluded_profiles,
        route_kind,
        !sync_probe_pressure_mode,
        sync_probe_pressure_mode,
        inflight_soft_limit,
        now,
    );
    let probe_counts = RuntimeResponseProbeCounts {
        stale_refreshes: probe_plan.stale_probe_refreshes.len(),
        cold_start_jobs: probe_plan.cold_start_probe_jobs.len(),
        sync_probe_jobs: probe_plan.sync_probe_jobs.len(),
        sync_probe_inline: probe_plan.should_sync_probe_cold_start,
    };
    schedule_runtime_response_stale_probes(shared, route_kind, &probe_plan);
    let (selection_state, reports, ready_candidates) =
        execute_runtime_response_probe_plan(shared, selection_state, probe_plan, route_kind, now)?;
    Ok(RuntimeResponseSelectionPrepared {
        pressure_mode,
        sync_probe_pressure_mode,
        inflight_soft_limit,
        selection_state,
        ready_candidates,
        report_count: reports.len(),
        probe_counts,
    })
}

fn load_runtime_route_selection_catalog(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Result<RuntimeRouteSelectionCatalog> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    Ok(runtime_route_selection_catalog(&runtime, route_kind, now))
}

fn schedule_runtime_response_stale_probes(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    probe_plan: &RuntimeResponseProbePlan,
) {
    for refresh in &probe_plan.stale_probe_refreshes {
        schedule_runtime_probe_refresh(shared, &refresh.name, &refresh.codex_home);
    }
    if let Some(skip_jobs) = probe_plan.sync_probe_skip_jobs_count {
        log_runtime_sync_probe_skip(shared, route_kind, "cold_start_jobs", skip_jobs);
    }
}

fn execute_runtime_response_probe_plan(
    shared: &RuntimeRotationProxyShared,
    selection_state: RuntimeRouteSelectionCatalog,
    probe_plan: RuntimeResponseProbePlan,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Result<(
    RuntimeRouteSelectionCatalog,
    Vec<RunProfileProbeReport>,
    Vec<ReadyProfileCandidate>,
)> {
    if probe_plan.should_sync_probe_cold_start {
        return execute_runtime_sync_probe_plan(
            shared,
            selection_state,
            probe_plan,
            route_kind,
            now,
        );
    }
    if let Some(skip_profiles) = probe_plan.sync_probe_skip_profiles_count {
        log_runtime_sync_probe_skip(shared, route_kind, "cold_start_profiles", skip_profiles);
    }
    for job in probe_plan.cold_start_probe_jobs {
        schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
    }
    Ok((
        selection_state,
        probe_plan.reports,
        probe_plan.ready_candidates,
    ))
}

fn execute_runtime_sync_probe_plan(
    shared: &RuntimeRotationProxyShared,
    selection_state: RuntimeRouteSelectionCatalog,
    probe_plan: RuntimeResponseProbePlan,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Result<(
    RuntimeRouteSelectionCatalog,
    Vec<RunProfileProbeReport>,
    Vec<ReadyProfileCandidate>,
)> {
    let base_url = Some(selection_state.upstream_base_url.clone());
    let upstream_no_proxy = shared.upstream_no_proxy;
    let sync_jobs = probe_plan
        .sync_probe_jobs
        .iter()
        .take(RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT)
        .cloned()
        .collect::<Vec<_>>();
    let probed_names = sync_jobs
        .iter()
        .map(|job| job.name.clone())
        .collect::<BTreeSet<_>>();
    let fresh_reports = map_parallel(sync_jobs, |job| {
        run_runtime_response_sync_probe(job, base_url.as_deref(), upstream_no_proxy)
    });
    for report in &fresh_reports {
        apply_runtime_profile_probe_result(
            shared,
            &report.name,
            report.auth.clone(),
            report.result.clone(),
        )?;
    }
    let selection_state = load_runtime_route_selection_catalog(shared, route_kind, now)?;
    let cached_usage_snapshots = selection_state.persisted_usage_snapshots();
    let mut reports = probe_plan.reports;
    for fresh_report in fresh_reports {
        if let Some(existing) = reports
            .iter_mut()
            .find(|report| report.name == fresh_report.name)
        {
            *existing = fresh_report;
        }
    }
    reports.sort_by_key(|report| report.order_index);
    let ready_candidates = ready_profile_candidates_with_view(
        &reports,
        selection_state.include_code_review,
        Some(selection_state.current_profile.as_str()),
        runtime_route_selection_view(&selection_state),
        Some(&cached_usage_snapshots),
    );
    for job in probe_plan
        .cold_start_probe_jobs
        .into_iter()
        .filter(|job| !probed_names.contains(&job.name))
    {
        schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
    }
    Ok((selection_state, reports, ready_candidates))
}

fn run_runtime_response_sync_probe(
    job: RunProfileProbeJob,
    base_url: Option<&str>,
    upstream_no_proxy: bool,
) -> RunProfileProbeReport {
    let auth = job.provider.auth_summary(&job.codex_home);
    let result = if auth.quota_compatible {
        fetch_usage_with_proxy_policy(&job.codex_home, base_url, upstream_no_proxy)
            .map_err(|err| err.to_string())
    } else {
        Err("auth mode is not quota-compatible".to_string())
    };
    RunProfileProbeReport {
        name: job.name,
        order_index: job.order_index,
        auth,
        result,
    }
}

fn log_runtime_sync_probe_skip(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    count_field: &'static str,
    count: usize,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "selection_skip_sync_probe",
            [
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("reason", "pressure_mode"),
                runtime_proxy_log_field(count_field, count.to_string()),
            ],
        ),
    );
}

pub(super) fn log_runtime_response_selection_plan(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    prompt_cache_owner: Option<&str>,
    prepared: &RuntimeResponseSelectionPrepared,
    candidate_plan: &RuntimeResponseCandidateExecutionPlan,
) {
    let counts = prepared.probe_counts;
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "selection_plan",
            [
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("pressure_mode", prepared.pressure_mode.to_string()),
                runtime_proxy_log_field(
                    "sync_probe_pressure",
                    prepared.sync_probe_pressure_mode.to_string(),
                ),
                runtime_proxy_log_field("reports", prepared.report_count.to_string()),
                runtime_proxy_log_field("ready", candidate_plan.ready_candidates.len().to_string()),
                runtime_proxy_log_field(
                    "fallback",
                    candidate_plan.fallback_candidates.len().to_string(),
                ),
                runtime_proxy_log_field("excluded_count", excluded_profiles.len().to_string()),
                runtime_proxy_log_field(
                    "inflight_soft_limit",
                    prepared.inflight_soft_limit.to_string(),
                ),
                runtime_proxy_log_field(
                    "stale_probe_refreshes",
                    counts.stale_refreshes.to_string(),
                ),
                runtime_proxy_log_field("cold_start_jobs", counts.cold_start_jobs.to_string()),
                runtime_proxy_log_field("sync_probe_jobs", counts.sync_probe_jobs.to_string()),
                runtime_proxy_log_field(
                    "sync_probe_mode",
                    if counts.sync_probe_inline {
                        "inline"
                    } else if counts.cold_start_jobs > 0 {
                        "background"
                    } else {
                        "none"
                    },
                ),
                runtime_proxy_log_field("prompt_cache_bound", prompt_cache_owner.unwrap_or("none")),
            ],
        ),
    );
}
