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
    let probe_plan =
        build_runtime_response_probe_plan(&selection_state, excluded_profiles, route_kind, now);
    let probe_counts = RuntimeResponseProbeCounts {
        stale_refreshes: probe_plan.stale_probe_refreshes.len(),
        cold_start_jobs: probe_plan.cold_start_probe_jobs.len(),
    };
    for refresh in &probe_plan.stale_probe_refreshes {
        schedule_runtime_probe_refresh(shared, &refresh.name, &refresh.codex_home);
    }
    for job in &probe_plan.cold_start_probe_jobs {
        schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
    }
    Ok(RuntimeResponseSelectionPrepared {
        pressure_mode,
        sync_probe_pressure_mode,
        inflight_soft_limit,
        selection_state,
        ready_candidates: probe_plan.ready_candidates,
        report_count: probe_plan.reports.len(),
        probe_counts,
    })
}

fn load_runtime_route_selection_catalog(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Result<RuntimeRouteSelectionCatalog> {
    let profile_inflight = shared.lane_admission.profile_inflight_snapshot();
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    Ok(runtime_route_selection_catalog(
        &runtime,
        &profile_inflight,
        route_kind,
        now,
    ))
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
                runtime_proxy_log_field(
                    "probe_mode",
                    if counts.cold_start_jobs > 0 {
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
