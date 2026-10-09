use super::*;

pub(crate) fn runtime_remaining_sync_probe_cold_start_profiles_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<usize> {
    let now = Local::now().timestamp();
    let profile_inflight = shared.lane_admission.profile_inflight_snapshot();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit =
        runtime_profile_inflight_soft_limit_for_shared(shared, route_kind, pressure_mode);
    let state = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        runtime_route_selection_catalog(&runtime, &profile_inflight, route_kind, now)
    };

    let mut count = 0;
    for name in active_profile_selection_order_with_view(
        runtime_route_selection_view(&state),
        &state.current_profile,
    ) {
        let Some(entry) = state.entry(&name) else {
            continue;
        };
        let hard_limited = runtime_profile_inflight_hard_limited_for_context(
            shared,
            &name,
            runtime_route_kind_inflight_context(route_kind),
        )?;
        let snapshot_blocks = entry
            .cached_usage_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                runtime_snapshot_blocks_same_request_cold_start_probe(snapshot, route_kind, now)
            });
        let eligible = runtime_waitable_candidate_eligible(
            prodex_mojo_core::runtime::WaitableCandidateMode::ColdStart,
            prodex_mojo_core::runtime::WaitableCandidateInput {
                context_allowed: !excluded_profiles.contains(&name),
                auth_compatible: !entry
                    .cached_auth_summary
                    .as_ref()
                    .is_some_and(|summary| !summary.quota_compatible),
                supports_runtime: entry.supports_codex_runtime(),
                cached_probe_present: entry.cached_probe_entry.is_some(),
                soft_limited: entry.inflight_count >= inflight_soft_limit,
                in_selection_backoff: entry.in_selection_backoff,
                auth_failure_active: entry.auth_failure_active,
                health_penalized: entry.health_sort_key > 0,
                hard_limited,
                snapshot_blocks,
                quota_blocked: false,
            },
        )?;
        if eligible {
            count += 1;
        }
    }
    Ok(count)
}

fn runtime_inflight_wait_quota_blocked(
    entry: &RuntimeRouteSelectionEntry,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    let live_probe_usage = entry
        .cached_probe_entry
        .as_ref()
        .and_then(|probe| probe.result.as_ref().ok());
    let (quota_summary, _) = runtime_quota_summary_from_cached_sources(
        live_probe_usage,
        entry.cached_usage_snapshot.as_ref(),
        route_kind,
        now,
        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    );
    runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some()
}

pub(crate) fn runtime_waitable_inflight_candidates_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    wait_affinity_owner: Option<&str>,
) -> Result<BTreeSet<String>> {
    let now = Local::now().timestamp();
    let profile_inflight = shared.lane_admission.profile_inflight_snapshot();
    let state = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_route_selection_catalog(&runtime, &profile_inflight, route_kind, now)
    };
    let mut waitable_profiles = BTreeSet::new();
    for name in active_profile_selection_order_with_view(
        runtime_route_selection_view(&state),
        &state.current_profile,
    ) {
        let Some(entry) = state.entry(&name) else {
            continue;
        };
        let hard_limited = runtime_profile_inflight_hard_limited_for_context(
            shared,
            &name,
            runtime_route_kind_inflight_context(route_kind),
        )?;
        let auth_compatible = !entry
            .cached_auth_summary
            .as_ref()
            .is_some_and(|auth| !auth.quota_compatible);
        let quota_blocked = runtime_inflight_wait_quota_blocked(entry, route_kind, now);
        let eligible = runtime_waitable_candidate_eligible(
            prodex_mojo_core::runtime::WaitableCandidateMode::Waitable,
            prodex_mojo_core::runtime::WaitableCandidateInput {
                context_allowed: !excluded_profiles.contains(&name)
                    && wait_affinity_owner.is_none_or(|owner| owner == name),
                auth_compatible,
                supports_runtime: entry.supports_codex_runtime(),
                cached_probe_present: entry.cached_probe_entry.is_some(),
                soft_limited: false,
                in_selection_backoff: entry.in_selection_backoff,
                auth_failure_active: entry.auth_failure_active,
                health_penalized: entry.health_sort_key > 0,
                hard_limited,
                snapshot_blocks: false,
                quota_blocked,
            },
        )?;
        if eligible {
            waitable_profiles.insert(name);
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
    let profile_inflight = shared.lane_admission.profile_inflight_snapshot();
    let state = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_route_selection_catalog(&runtime, &profile_inflight, route_kind, now)
    };
    for name in waited_profiles {
        let Some(entry) = state.entry(name) else {
            continue;
        };
        let hard_limited = runtime_profile_inflight_hard_limited_for_context(
            shared,
            name,
            runtime_route_kind_inflight_context(route_kind),
        )?;
        let auth_compatible = !entry
            .cached_auth_summary
            .as_ref()
            .is_some_and(|auth| !auth.quota_compatible);
        let quota_blocked = runtime_inflight_wait_quota_blocked(entry, route_kind, now);
        let eligible = runtime_waitable_candidate_eligible(
            prodex_mojo_core::runtime::WaitableCandidateMode::Relieved,
            prodex_mojo_core::runtime::WaitableCandidateInput {
                context_allowed: true,
                auth_compatible,
                supports_runtime: entry.supports_codex_runtime(),
                cached_probe_present: entry.cached_probe_entry.is_some(),
                soft_limited: false,
                in_selection_backoff: entry.in_selection_backoff,
                auth_failure_active: entry.auth_failure_active,
                health_penalized: entry.health_sort_key > 0,
                hard_limited,
                snapshot_blocks: false,
                quota_blocked,
            },
        )?;
        if eligible {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(crate) struct RuntimeInflightReliefWait<'a> {
    pub(crate) observed_release_revision: Option<u64>,
    pub(crate) request_id: u64,
    pub(crate) shared: &'a RuntimeRotationProxyShared,
    pub(crate) excluded_profiles: &'a BTreeSet<String>,
    pub(crate) route_kind: RuntimeRouteKind,
    pub(crate) selection_started_at: &'a mut Instant,
    pub(crate) continuation: bool,
    pub(crate) wait_affinity_owner: Option<&'a str>,
    pub(crate) selected_profile: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeInflightReliefWaitResult {
    NotWaitable,
    Relieved,
}

struct RuntimeInflightWaitState {
    observed_revision: u64,
    observed_selection_revision: u64,
    signaled: bool,
    useful_relief: bool,
    wake_source: RuntimeProfileInFlightWaitOutcome,
}

fn runtime_scoped_waitable_profiles(
    wait: &RuntimeInflightReliefWait<'_>,
) -> Result<BTreeSet<String>> {
    let mut waited_profiles = runtime_waitable_inflight_candidates_for_route(
        wait.shared,
        wait.excluded_profiles,
        wait.route_kind,
        wait.wait_affinity_owner,
    )?;
    if let Some(selected_profile) = wait.selected_profile {
        waited_profiles.retain(|profile| profile == selected_profile);
    }
    Ok(waited_profiles)
}

fn log_runtime_inflight_wait_started(
    wait: &RuntimeInflightReliefWait<'_>,
    wait_epoch: Duration,
    waited_profiles: &BTreeSet<String>,
) {
    runtime_proxy_log(
        wait.shared,
        runtime_proxy_structured_log_message(
            "inflight_wait_started",
            [
                runtime_proxy_log_field("route", runtime_route_kind_label(wait.route_kind)),
                runtime_proxy_log_field("request", wait.request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("wait_ms", wait_epoch.as_millis().to_string()),
                runtime_proxy_log_field(
                    "selection_elapsed_ms",
                    wait.selection_started_at.elapsed().as_millis().to_string(),
                ),
                runtime_proxy_log_field(
                    "eligible_candidate_count",
                    waited_profiles.len().to_string(),
                ),
                runtime_proxy_log_field(
                    "saturated_candidate_count",
                    waited_profiles.len().to_string(),
                ),
                runtime_proxy_log_field(
                    "waiter_priority",
                    if wait.continuation {
                        "continuation"
                    } else {
                        "normal"
                    },
                ),
                runtime_proxy_log_field("deadline_ms", "none"),
                runtime_proxy_log_field("mode", "backpressure"),
            ],
        ),
    );
}

fn refresh_runtime_inflight_wait_candidates(
    wait: &RuntimeInflightReliefWait<'_>,
    started_at: Instant,
    state: &mut RuntimeInflightWaitState,
) -> Result<Option<BTreeSet<String>>> {
    state.observed_revision = runtime_profile_inflight_release_revision(wait.shared);
    state.observed_selection_revision = wait.shared.lane_admission.selection_change_revision();
    let refreshed = runtime_scoped_waitable_profiles(wait)?;
    if refreshed.is_empty() {
        return Ok(None);
    }
    runtime_proxy_log(
        wait.shared,
        runtime_proxy_structured_log_message(
            "local_capacity_wait_continued",
            [
                runtime_proxy_log_field("route", runtime_route_kind_label(wait.route_kind)),
                runtime_proxy_log_field("request", wait.request_id.to_string()),
                runtime_proxy_log_field("waited_ms", started_at.elapsed().as_millis().to_string()),
                runtime_proxy_log_field("eligible_candidate_count", refreshed.len().to_string()),
                runtime_proxy_log_field("mode", "backpressure"),
            ],
        ),
    );
    Ok(Some(refreshed))
}

fn run_runtime_inflight_backpressure_wait(
    wait: &RuntimeInflightReliefWait<'_>,
    wait_epoch: Duration,
    started_at: Instant,
    waited_profiles: &mut BTreeSet<String>,
    state: &mut RuntimeInflightWaitState,
) -> Result<bool> {
    loop {
        // Notifications are hints, not readiness. In particular, a release
        // can precede the waiter reaching the condition variable.
        if runtime_any_waited_candidate_relieved(wait.shared, waited_profiles, wait.route_kind)? {
            state.useful_relief = true;
            state.wake_source = RuntimeProfileInFlightWaitOutcome::InflightRelease;
            return Ok(true);
        }
        let outcome = runtime_profile_inflight_wait_outcome_since_with_selection_revision(
            wait.shared,
            wait_epoch,
            state.observed_revision,
            state.observed_selection_revision,
        );
        if matches!(outcome, RuntimeProfileInFlightWaitOutcome::Timeout) {
            let Some(refreshed) =
                refresh_runtime_inflight_wait_candidates(wait, started_at, state)?
            else {
                // The old saturated pool disappeared while waiting. It may
                // now be ready, or its eligibility changed: reselect rather
                // than treating that stale snapshot as terminal exhaustion.
                state.useful_relief = true;
                state.wake_source = RuntimeProfileInFlightWaitOutcome::SelectionChanged;
                return Ok(true);
            };
            *waited_profiles = refreshed;
            continue;
        }
        if process_runtime_inflight_wait_outcome(
            wait.shared,
            waited_profiles,
            wait.route_kind,
            wait.request_id,
            started_at,
            state,
            outcome,
        )? {
            return Ok(true);
        }
    }
}

fn log_runtime_inflight_wait_finished(
    wait: &RuntimeInflightReliefWait<'_>,
    started_at: Instant,
    state: &RuntimeInflightWaitState,
) {
    runtime_proxy_log(
        wait.shared,
        runtime_proxy_structured_log_message(
            "inflight_wait_finished",
            [
                runtime_proxy_log_field("route", runtime_route_kind_label(wait.route_kind)),
                runtime_proxy_log_field("request", wait.request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("waited_ms", started_at.elapsed().as_millis().to_string()),
                runtime_proxy_log_field("signaled", state.signaled.to_string()),
                runtime_proxy_log_field("useful", state.useful_relief.to_string()),
                runtime_proxy_log_field(
                    "wake_source",
                    runtime_profile_inflight_wait_outcome_label(state.wake_source),
                ),
            ],
        ),
    );
}

pub(crate) fn runtime_proxy_maybe_wait_for_interactive_inflight_relief(
    wait: RuntimeInflightReliefWait<'_>,
) -> Result<RuntimeInflightReliefWaitResult> {
    // Register revisions before inspecting readiness: a concurrent release
    // must be visible either in the predicate or in the subsequent wait.
    let mut wait_state = RuntimeInflightWaitState {
        observed_revision: runtime_profile_inflight_release_revision(wait.shared),
        observed_selection_revision: wait.shared.lane_admission.selection_change_revision(),
        signaled: false,
        useful_relief: false,
        wake_source: RuntimeProfileInFlightWaitOutcome::Timeout,
    };
    let mut waited_profiles = runtime_scoped_waitable_profiles(&wait)?;
    if waited_profiles.is_empty() {
        // Selection may have seen a full pool just before a permit release.
        // An empty *saturated* set then means reselection, not pool exhaustion.
        if wait.observed_release_revision.is_some_and(|revision| {
            revision != runtime_profile_inflight_release_revision(wait.shared)
        }) {
            return Ok(RuntimeInflightReliefWaitResult::Relieved);
        }
        return Ok(RuntimeInflightReliefWaitResult::NotWaitable);
    }

    let wait_epoch = if cfg!(test) {
        Duration::from_millis(1_500)
    } else {
        Duration::from_millis(runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS)
    };
    log_runtime_inflight_wait_started(&wait, wait_epoch, &waited_profiles);

    let started_at = Instant::now();
    let result = run_runtime_inflight_backpressure_wait(
        &wait,
        wait_epoch,
        started_at,
        &mut waited_profiles,
        &mut wait_state,
    );
    // Local contention is not an upstream attempt. Pause only the monotonic
    // retry clock; never clear exclusions, failures, or the attempt counter.
    if let Some(resumed_at) = wait.selection_started_at.checked_add(started_at.elapsed()) {
        *wait.selection_started_at = resumed_at;
    }
    let remained_waitable = result?;
    log_runtime_inflight_wait_finished(&wait, started_at, &wait_state);

    match (wait_state.useful_relief, remained_waitable) {
        (true, _) => Ok(RuntimeInflightReliefWaitResult::Relieved),
        (false, false) => Ok(RuntimeInflightReliefWaitResult::NotWaitable),
        (false, true) => unreachable!(
            "inflight backpressure wait exits only after relief or eligibility changes"
        ),
    }
}

fn process_runtime_inflight_wait_outcome(
    shared: &RuntimeRotationProxyShared,
    waited_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    request_id: u64,
    started_at: Instant,
    state: &mut RuntimeInflightWaitState,
    outcome: RuntimeProfileInFlightWaitOutcome,
) -> Result<bool> {
    match outcome {
        RuntimeProfileInFlightWaitOutcome::InflightRelease => {
            state.signaled = true;
            state.wake_source = RuntimeProfileInFlightWaitOutcome::InflightRelease;
            state.observed_revision = runtime_profile_inflight_release_revision(shared);
            state.observed_selection_revision = shared.lane_admission.selection_change_revision();
            state.useful_relief =
                runtime_any_waited_candidate_relieved(shared, waited_profiles, route_kind)?;
            if state.useful_relief {
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "local_capacity_wait_woke",
                        [
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("wake_reason", "capacity_released"),
                            runtime_proxy_log_field(
                                "waited_ms",
                                started_at.elapsed().as_millis().to_string(),
                            ),
                        ],
                    ),
                );
                return Ok(true);
            }
        }
        RuntimeProfileInFlightWaitOutcome::OtherNotify => {
            state.signaled = true;
            state.wake_source = RuntimeProfileInFlightWaitOutcome::OtherNotify;
            state.observed_revision = runtime_profile_inflight_release_revision(shared);
            state.observed_selection_revision = shared.lane_admission.selection_change_revision();
            state.useful_relief =
                runtime_any_waited_candidate_relieved(shared, waited_profiles, route_kind)?;
            if state.useful_relief {
                return Ok(true);
            }
        }
        RuntimeProfileInFlightWaitOutcome::SelectionChanged => {
            state.signaled = true;
            state.wake_source = RuntimeProfileInFlightWaitOutcome::SelectionChanged;
            state.useful_relief = true;
            return Ok(true);
        }
        RuntimeProfileInFlightWaitOutcome::Timeout => {
            if !state.signaled {
                state.wake_source = RuntimeProfileInFlightWaitOutcome::Timeout;
            }
            return Ok(true);
        }
    }
    Ok(false)
}
