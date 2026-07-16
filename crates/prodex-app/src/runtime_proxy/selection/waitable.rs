use super::*;

pub(crate) fn runtime_remaining_sync_probe_cold_start_profiles_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<usize> {
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let now = Local::now().timestamp();
    let profile_inflight = shared.lane_admission.profile_inflight_snapshot();
    let state = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        runtime_route_selection_catalog(&runtime, &profile_inflight, route_kind, now)
    };

    Ok(active_profile_selection_order_with_view(
        runtime_route_selection_view(&state),
        &state.current_profile,
    )
    .into_iter()
    .filter(|name| !excluded_profiles.contains(name))
    .filter(|name| {
        state.entry(name).is_some_and(|entry| {
            entry
                .cached_auth_summary
                .clone()
                .or_else(|| {
                    allow_disk_auth_fallback.then(|| read_auth_summary(&entry.profile.codex_home))
                })
                .is_some_and(|summary| summary.quota_compatible)
        })
    })
    .filter(|name| {
        state
            .entry(name)
            .is_some_and(|entry| entry.cached_probe_entry.is_none())
    })
    .filter(|name| {
        !state.entry(name).is_some_and(|entry| {
            entry
                .cached_usage_snapshot
                .as_ref()
                .is_some_and(|snapshot| {
                    runtime_snapshot_blocks_same_request_cold_start_probe(snapshot, route_kind, now)
                })
        })
    })
    .count())
}

pub(crate) fn runtime_waitable_inflight_candidates_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    wait_affinity_owner: Option<&str>,
) -> Result<BTreeSet<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit =
        runtime_profile_inflight_soft_limit_for_shared(shared, route_kind, pressure_mode);
    let profile_inflight = shared.lane_admission.profile_inflight_snapshot();
    let state = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_route_selection_catalog(&runtime, &profile_inflight, route_kind, now)
    };
    let cached_usage_snapshots = state.persisted_usage_snapshots();

    let mut waitable_profiles = BTreeSet::new();
    let mut reports = Vec::new();
    for (order_index, name) in active_profile_selection_order_with_view(
        runtime_route_selection_view(&state),
        &state.current_profile,
    )
    .into_iter()
    .enumerate()
    {
        if excluded_profiles.contains(&name) {
            continue;
        }
        if wait_affinity_owner.is_some_and(|owner| owner != name) {
            continue;
        }
        let Some(entry) = state.entry(&name) else {
            continue;
        };
        let Some(probe_entry) = entry.cached_probe_entry.as_ref() else {
            continue;
        };
        reports.push(RunProfileProbeReport {
            name,
            order_index,
            auth: probe_entry.auth.clone(),
            result: probe_entry.result.clone(),
        });
    }

    for candidate in ready_profile_candidates_with_view(
        &reports,
        state.include_code_review,
        Some(state.current_profile.as_str()),
        runtime_route_selection_view(&state),
        Some(&cached_usage_snapshots),
    ) {
        if excluded_profiles.contains(&candidate.name) {
            continue;
        }
        let Some(entry) = state.entry(&candidate.name) else {
            continue;
        };
        if entry.in_selection_backoff || entry.auth_failure_active {
            continue;
        }
        if runtime_quota_precommit_guard_reason(
            runtime_quota_summary_for_route(&candidate.usage, route_kind),
            route_kind,
        )
        .is_some()
        {
            continue;
        }
        if entry.inflight_count >= inflight_soft_limit {
            waitable_profiles.insert(candidate.name.clone());
        }
    }

    Ok(waitable_profiles)
}

pub(crate) fn runtime_any_waited_candidate_relieved(
    shared: &RuntimeRotationProxyShared,
    waited_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    if waited_profiles.is_empty() {
        return Ok(false);
    }
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit =
        runtime_profile_inflight_soft_limit_for_shared(shared, route_kind, pressure_mode);
    let profile_inflight = shared.lane_admission.profile_inflight_snapshot();
    let state = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_route_selection_catalog(&runtime, &profile_inflight, route_kind, now)
    };
    let cached_usage_snapshots = state.persisted_usage_snapshots();

    let mut reports = Vec::new();
    for (order_index, name) in active_profile_selection_order_with_view(
        runtime_route_selection_view(&state),
        &state.current_profile,
    )
    .into_iter()
    .enumerate()
    {
        if !waited_profiles.contains(&name) {
            continue;
        }
        let Some(entry) = state.entry(&name) else {
            continue;
        };
        let Some(probe_entry) = entry.cached_probe_entry.as_ref() else {
            continue;
        };
        reports.push(RunProfileProbeReport {
            name,
            order_index,
            auth: probe_entry.auth.clone(),
            result: probe_entry.result.clone(),
        });
    }

    for candidate in ready_profile_candidates_with_view(
        &reports,
        state.include_code_review,
        Some(state.current_profile.as_str()),
        runtime_route_selection_view(&state),
        Some(&cached_usage_snapshots),
    ) {
        if !waited_profiles.contains(&candidate.name) {
            continue;
        }
        let Some(entry) = state.entry(&candidate.name) else {
            continue;
        };
        if entry.in_selection_backoff || entry.auth_failure_active {
            continue;
        }
        if runtime_quota_precommit_guard_reason(
            runtime_quota_summary_for_route(&candidate.usage, route_kind),
            route_kind,
        )
        .is_some()
        {
            continue;
        }
        if entry.inflight_count < inflight_soft_limit {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(crate) struct RuntimeInflightReliefWait<'a> {
    pub(crate) request_id: u64,
    pub(crate) request: &'a RuntimeProxyRequest,
    pub(crate) shared: &'a RuntimeRotationProxyShared,
    pub(crate) excluded_profiles: &'a BTreeSet<String>,
    pub(crate) route_kind: RuntimeRouteKind,
    pub(crate) selection_started_at: Instant,
    pub(crate) continuation: bool,
    pub(crate) wait_affinity_owner: Option<&'a str>,
}

pub(crate) fn runtime_proxy_maybe_wait_for_interactive_inflight_relief(
    wait: RuntimeInflightReliefWait<'_>,
) -> Result<bool> {
    let RuntimeInflightReliefWait {
        request_id,
        request,
        shared,
        excluded_profiles,
        route_kind,
        selection_started_at,
        continuation,
        wait_affinity_owner,
    } = wait;

    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let wait_budget =
        runtime_proxy_request_inflight_wait_budget(request, pressure_mode, &shared.runtime_config);
    if wait_budget.is_zero() {
        return Ok(false);
    }
    let waited_profiles = runtime_waitable_inflight_candidates_for_route(
        shared,
        excluded_profiles,
        route_kind,
        wait_affinity_owner,
    )?;
    if waited_profiles.is_empty() {
        return Ok(false);
    }

    let (_, precommit_budget) = runtime_proxy_precommit_budget(continuation, pressure_mode);
    let remaining_budget = precommit_budget.saturating_sub(selection_started_at.elapsed());
    let total_wait_budget = wait_budget.min(remaining_budget);
    if total_wait_budget.is_zero() {
        return Ok(false);
    }
    let wait_deadline = Instant::now() + total_wait_budget;

    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "inflight_wait_started",
            [
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("wait_ms", total_wait_budget.as_millis().to_string()),
            ],
        ),
    );
    let started_at = Instant::now();
    let mut observed_revision = runtime_profile_inflight_release_revision(shared);
    let mut signaled = false;
    let mut useful_relief = false;
    let mut wake_source = RuntimeProfileInFlightWaitOutcome::Timeout;
    loop {
        let remaining_wait = wait_deadline.saturating_duration_since(Instant::now());
        if remaining_wait.is_zero() {
            break;
        }
        match runtime_profile_inflight_wait_outcome_since(shared, remaining_wait, observed_revision)
        {
            RuntimeProfileInFlightWaitOutcome::InflightRelease => {
                signaled = true;
                wake_source = RuntimeProfileInFlightWaitOutcome::InflightRelease;
                observed_revision = runtime_profile_inflight_release_revision(shared);
                useful_relief =
                    runtime_any_waited_candidate_relieved(shared, &waited_profiles, route_kind)?;
                if useful_relief {
                    break;
                }
            }
            RuntimeProfileInFlightWaitOutcome::OtherNotify => {
                signaled = true;
                wake_source = RuntimeProfileInFlightWaitOutcome::OtherNotify;
                observed_revision = runtime_profile_inflight_release_revision(shared);
            }
            RuntimeProfileInFlightWaitOutcome::Timeout => {
                if !signaled {
                    wake_source = RuntimeProfileInFlightWaitOutcome::Timeout;
                }
                break;
            }
        }
    }
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "inflight_wait_finished",
            [
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("waited_ms", started_at.elapsed().as_millis().to_string()),
                runtime_proxy_log_field("signaled", signaled.to_string()),
                runtime_proxy_log_field("useful", useful_relief.to_string()),
                runtime_proxy_log_field(
                    "wake_source",
                    runtime_profile_inflight_wait_outcome_label(wake_source),
                ),
            ],
        ),
    );
    Ok(useful_relief)
}
