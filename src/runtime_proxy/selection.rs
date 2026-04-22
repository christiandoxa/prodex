use super::*;

pub(crate) fn runtime_proxy_sync_probe_pressure_pause(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
) {
    if !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind) {
        return;
    }
    let pause_ms = RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS;
    let observed_revision = runtime_probe_refresh_revision();
    let started_at = Instant::now();
    let wait_outcome = runtime_probe_refresh_wait_outcome_since(
        Duration::from_millis(pause_ms),
        observed_revision,
    );
    runtime_proxy_log(
        shared,
        format!(
            "runtime_proxy_sync_probe_pressure_pause route={} pause_ms={} waited_ms={} outcome={}",
            runtime_route_kind_label(route_kind),
            pause_ms,
            started_at.elapsed().as_millis(),
            runtime_profile_wait_outcome_label(wait_outcome),
        ),
    );
}

pub(crate) fn runtime_previous_response_affinity_is_trusted(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
) -> Result<bool> {
    let Some(previous_response_id) = previous_response_id else {
        return Ok(false);
    };
    let Some(bound_profile) = bound_profile else {
        return Ok(false);
    };

    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let Some(binding) = runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
    else {
        return Ok(false);
    };
    if binding.profile_name != bound_profile {
        return Ok(false);
    }
    Ok(runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
    )
    .get(previous_response_id)
    .is_none_or(|status| {
        status.state == RuntimeContinuationBindingLifecycle::Verified
            || (status.state == RuntimeContinuationBindingLifecycle::Warm
                && status.last_verified_at.is_some())
    }))
}

pub(crate) fn runtime_previous_response_affinity_is_bound(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
) -> Result<bool> {
    let Some(previous_response_id) = previous_response_id else {
        return Ok(false);
    };
    let Some(bound_profile) = bound_profile else {
        return Ok(false);
    };

    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .is_some_and(|binding| binding.profile_name == bound_profile))
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeCandidateAffinity<'a> {
    pub(crate) route_kind: RuntimeRouteKind,
    pub(crate) candidate_name: &'a str,
    pub(crate) strict_affinity_profile: Option<&'a str>,
    pub(crate) pinned_profile: Option<&'a str>,
    pub(crate) turn_state_profile: Option<&'a str>,
    pub(crate) session_profile: Option<&'a str>,
    pub(crate) trusted_previous_response_affinity: bool,
}

impl<'a> RuntimeCandidateAffinity<'a> {
    #[allow(dead_code)]
    pub(crate) fn new(
        route_kind: RuntimeRouteKind,
        candidate_name: &'a str,
        strict_affinity_profile: Option<&'a str>,
        pinned_profile: Option<&'a str>,
        turn_state_profile: Option<&'a str>,
        session_profile: Option<&'a str>,
        trusted_previous_response_affinity: bool,
    ) -> Self {
        Self {
            route_kind,
            candidate_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity,
        }
    }
}

pub(crate) fn runtime_candidate_has_hard_affinity(affinity: RuntimeCandidateAffinity<'_>) -> bool {
    affinity
        .strict_affinity_profile
        .is_some_and(|profile_name| profile_name == affinity.candidate_name)
        || affinity
            .turn_state_profile
            .is_some_and(|profile_name| profile_name == affinity.candidate_name)
        || (affinity.trusted_previous_response_affinity
            && affinity
                .pinned_profile
                .is_some_and(|profile_name| profile_name == affinity.candidate_name))
        || (affinity.route_kind == RuntimeRouteKind::Compact
            && affinity
                .session_profile
                .is_some_and(|profile_name| profile_name == affinity.candidate_name))
}

pub(crate) fn runtime_quota_blocked_affinity_is_releasable(
    affinity: RuntimeCandidateAffinity<'_>,
    _request_requires_previous_response_affinity: bool,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    if fresh_fallback_shape.is_some() {
        // Fail closed: previous_response continuations stay chained to the owning profile.
        return false;
    }

    if affinity
        .strict_affinity_profile
        .is_some_and(|profile_name| profile_name == affinity.candidate_name)
        || affinity
            .turn_state_profile
            .is_some_and(|profile_name| profile_name == affinity.candidate_name)
        || (affinity.route_kind == RuntimeRouteKind::Compact
            && affinity
                .session_profile
                .is_some_and(|profile_name| profile_name == affinity.candidate_name))
    {
        return false;
    }

    if affinity.trusted_previous_response_affinity
        && affinity
            .pinned_profile
            .is_some_and(|profile_name| profile_name == affinity.candidate_name)
    {
        return true;
    }

    true
}

pub(crate) fn runtime_quota_blocked_previous_response_fresh_fallback_allowed(
    _previous_response_id: Option<&str>,
    _trusted_previous_response_affinity: bool,
    _previous_response_fresh_fallback_used: bool,
    _fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    false
}

pub(crate) fn runtime_websocket_previous_response_reuse_is_nonreplayable(
    previous_response_id: Option<&str>,
    previous_response_fresh_fallback_used: bool,
    turn_state_override: Option<&str>,
) -> bool {
    previous_response_id.is_some()
        && !previous_response_fresh_fallback_used
        && turn_state_override.is_none()
}

pub(crate) fn runtime_websocket_previous_response_reuse_is_stale(
    nonreplayable_previous_response_reuse: bool,
    reuse_terminal_idle: Option<Duration>,
) -> bool {
    nonreplayable_previous_response_reuse
        && reuse_terminal_idle.is_some_and(|elapsed| {
            elapsed
                >= Duration::from_millis(runtime_proxy_websocket_previous_response_reuse_stale_ms())
        })
}

#[allow(dead_code)]
pub(crate) fn runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed(
    _request: RuntimeWebsocketReuseWatchdogPreviousResponseFallback<'_>,
) -> bool {
    false
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) struct RuntimeWebsocketReuseWatchdogPreviousResponseFallback<'a> {
    pub(crate) profile_name: &'a str,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) previous_response_fresh_fallback_used: bool,
    pub(crate) bound_profile: Option<&'a str>,
    pub(crate) pinned_profile: Option<&'a str>,
    pub(crate) request_requires_previous_response_affinity: bool,
    pub(crate) trusted_previous_response_affinity: bool,
    pub(crate) request_turn_state: Option<&'a str>,
    pub(crate) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

pub(crate) fn runtime_websocket_previous_response_not_found_requires_stale_continuation(
    previous_response_id: Option<&str>,
    has_turn_state_retry: bool,
    _request_requires_locked_previous_response_affinity: bool,
    _fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    previous_response_id.is_some() && !has_turn_state_retry
}

pub(crate) fn runtime_previous_response_not_found_fresh_fallback_allowed(
    _previous_response_id: Option<&str>,
    _previous_response_fresh_fallback_used: bool,
    _request_requires_locked_previous_response_affinity: bool,
    _fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    false
}

pub(crate) fn runtime_quota_precommit_floor_percent(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => {
            runtime_proxy_responses_quota_critical_floor_percent()
        }
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 1,
    }
}

pub(crate) fn runtime_quota_window_precommit_guard(
    window: RuntimeQuotaWindowSummary,
    floor_percent: i64,
) -> bool {
    matches!(
        window.status,
        RuntimeQuotaWindowStatus::Critical | RuntimeQuotaWindowStatus::Exhausted
    ) && window.remaining_percent <= floor_percent
}

pub(crate) fn runtime_quota_precommit_guard_reason(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<&'static str> {
    let floor_percent = runtime_quota_precommit_floor_percent(route_kind);
    if summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        return Some("quota_exhausted_before_send");
    }

    if matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && (runtime_quota_window_precommit_guard(summary.five_hour, floor_percent)
        || runtime_quota_window_precommit_guard(summary.weekly, floor_percent))
    {
        return Some("quota_critical_floor_before_send");
    }

    None
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeResponseCandidateSelection<'a> {
    pub(crate) excluded_profiles: &'a BTreeSet<String>,
    pub(crate) strict_affinity_profile: Option<&'a str>,
    pub(crate) pinned_profile: Option<&'a str>,
    pub(crate) turn_state_profile: Option<&'a str>,
    pub(crate) session_profile: Option<&'a str>,
    pub(crate) discover_previous_response_owner: bool,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) route_kind: RuntimeRouteKind,
}

#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn select_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    strict_affinity_profile: Option<&str>,
    pinned_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    discover_previous_response_owner: bool,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    select_runtime_response_candidate_for_route_with_selection(
        shared,
        RuntimeResponseCandidateSelection {
            excluded_profiles,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            discover_previous_response_owner,
            previous_response_id,
            route_kind,
        },
    )
}

pub(crate) fn select_runtime_response_candidate_for_route_with_selection(
    shared: &RuntimeRotationProxyShared,
    selection: RuntimeResponseCandidateSelection<'_>,
) -> Result<Option<String>> {
    let RuntimeResponseCandidateSelection {
        excluded_profiles,
        strict_affinity_profile,
        pinned_profile,
        turn_state_profile,
        session_profile,
        discover_previous_response_owner,
        previous_response_id,
        route_kind,
    } = selection;

    if let Some(profile_name) = strict_affinity_profile {
        if excluded_profiles.contains(profile_name) {
            return Ok(None);
        }
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: false,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        let compact_followup_owner_without_probe = matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && quota_source.is_none();
        if runtime_quota_summary_allows_soft_affinity(quota_summary, quota_source, route_kind)
            || compact_followup_owner_without_probe
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=compact_followup profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_soft_affinity_rejection_reason(
                    quota_summary,
                    quota_source,
                    route_kind,
                ),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }

    if let Some(profile_name) = pinned_profile.filter(|name| !excluded_profiles.contains(*name)) {
        if runtime_previous_response_affinity_is_bound(
            shared,
            previous_response_id,
            pinned_profile,
        )? {
            return Ok(Some(profile_name.to_string()));
        }
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: runtime_previous_response_affinity_is_trusted(
                shared,
                previous_response_id,
                pinned_profile,
            )?,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        if quota_summary.route_band <= RuntimeQuotaPressureBand::Critical
            && runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_none()
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=pinned profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_pressure_band_reason(quota_summary.route_band),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if let Some(profile_name) = turn_state_profile.filter(|name| !excluded_profiles.contains(*name))
    {
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: false,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        if quota_summary.route_band <= RuntimeQuotaPressureBand::Critical
            && runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_none()
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=turn_state profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_pressure_band_reason(quota_summary.route_band),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if discover_previous_response_owner {
        return next_runtime_previous_response_candidate(
            shared,
            excluded_profiles,
            previous_response_id,
            route_kind,
        );
    }

    if let Some(profile_name) = session_profile.filter(|name| !excluded_profiles.contains(*name)) {
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: false,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        let compact_session_owner_without_probe =
            route_kind == RuntimeRouteKind::Compact && quota_source.is_none();
        let websocket_unknown_current_profile_without_pool_fallback = route_kind
            == RuntimeRouteKind::Websocket
            && quota_source.is_none()
            && runtime_proxy_current_profile(shared)? == profile_name
            && !runtime_has_route_eligible_quota_fallback(
                shared,
                profile_name,
                excluded_profiles,
                route_kind,
            )?;
        if runtime_quota_summary_allows_soft_affinity(quota_summary, quota_source, route_kind)
            || compact_session_owner_without_probe
            || websocket_unknown_current_profile_without_pool_fallback
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=session profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_soft_affinity_rejection_reason(
                    quota_summary,
                    quota_source,
                    route_kind,
                ),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if let Some(profile_name) =
        runtime_proxy_optimistic_current_candidate_for_route(shared, excluded_profiles, route_kind)?
    {
        return Ok(Some(profile_name));
    }

    next_runtime_response_candidate_for_route(shared, excluded_profiles, route_kind)
}

pub(crate) fn next_runtime_previous_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let (state, current_profile, profile_health, profile_usage_auth, profile_probe_cache) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.profile_health.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_probe_cache.clone(),
        )
    };
    let now = Local::now().timestamp();
    if let Some(previous_response_id) = previous_response_id
        && let Some(binding) = state.response_profile_bindings.get(previous_response_id)
    {
        let owner = binding.profile_name.as_str();
        if !excluded_profiles.contains(owner)
            && state.profiles.contains_key(owner)
            && !runtime_previous_response_negative_cache_active(
                &profile_health,
                previous_response_id,
                owner,
                route_kind,
                now,
            )
        {
            return Ok(Some(owner.to_string()));
        }
    }

    for name in active_profile_selection_order(&state, &current_profile) {
        if excluded_profiles.contains(&name) {
            continue;
        }
        if let Some(previous_response_id) = previous_response_id
            && runtime_previous_response_negative_cache_active(
                &profile_health,
                previous_response_id,
                &name,
                route_kind,
                now,
            )
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason=negative_cache response_id={}",
                    runtime_route_kind_label(route_kind),
                    name,
                    previous_response_id,
                ),
            );
            continue;
        }
        let Some(profile) = state.profiles.get(&name) else {
            continue;
        };
        if !runtime_profile_auth_summary_for_selection_with_policy(
            &name,
            &profile.codex_home,
            &profile_usage_auth,
            &profile_probe_cache,
            allow_disk_auth_fallback,
        )
        .is_some_and(|summary| summary.quota_compatible)
        {
            continue;
        }
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &name,
            now,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason=auth_failure_backoff",
                    runtime_route_kind_label(route_kind),
                    name,
                ),
            );
            continue;
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, &name, route_kind)?;
        if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted
            || runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some()
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    name,
                    runtime_quota_precommit_guard_reason(quota_summary, route_kind).unwrap_or_else(
                        || runtime_quota_pressure_band_reason(quota_summary.route_band)
                    ),
                    quota_source
                        .map(runtime_quota_source_label)
                        .unwrap_or("unknown"),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        return Ok(Some(name));
    }
    Ok(None)
}

pub(crate) fn runtime_proxy_optimistic_current_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let pressure_mode = runtime_proxy_pressure_mode_active(shared);
    let (
        current_profile,
        codex_home,
        in_selection_backoff,
        circuit_open_until,
        inflight_count,
        health_score,
        performance_score,
        auth_failure_active,
        cached_usage_auth_entry,
        probe_cache_entry,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let now = Local::now().timestamp();
        prune_runtime_profile_selection_backoff(&mut runtime, now);

        if excluded_profiles.contains(&runtime.current_profile) {
            return Ok(None);
        }

        let Some(profile) = runtime.state.profiles.get(&runtime.current_profile) else {
            return Ok(None);
        };
        (
            runtime.current_profile.clone(),
            profile.codex_home.clone(),
            runtime_profile_in_selection_backoff(
                &runtime,
                &runtime.current_profile,
                route_kind,
                now,
            ),
            runtime_profile_route_circuit_open_until(
                &runtime,
                &runtime.current_profile,
                route_kind,
                now,
            ),
            runtime_profile_inflight_count(&runtime, &runtime.current_profile),
            runtime_profile_health_score(&runtime, &runtime.current_profile, now, route_kind),
            runtime_profile_route_performance_score(
                &runtime.profile_health,
                &runtime.current_profile,
                now,
                route_kind,
            ),
            runtime_profile_auth_failure_active_with_auth_cache(
                &runtime.profile_health,
                &runtime.profile_usage_auth,
                &runtime.current_profile,
                now,
            ),
            runtime
                .profile_usage_auth
                .get(&runtime.current_profile)
                .cloned(),
            runtime
                .profile_probe_cache
                .get(&runtime.current_profile)
                .cloned(),
        )
    };
    let has_alternative_quota_compatible_profile = runtime_has_route_eligible_quota_fallback(
        shared,
        &current_profile,
        excluded_profiles,
        route_kind,
    )?;
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let current_profile_quota_compatible = runtime_profile_cached_auth_summary_for_selection(
        cached_usage_auth_entry,
        probe_cache_entry,
    )
    .unwrap_or_else(|| {
        if allow_disk_auth_fallback {
            read_auth_summary(&codex_home)
        } else {
            AuthSummary {
                label: "uncached-auth".to_string(),
                quota_compatible: false,
            }
        }
    })
    .quota_compatible;
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, &current_profile, route_kind)?;
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let quota_evidence_required =
        has_alternative_quota_compatible_profile && quota_source.is_none();
    let live_quota_probe_required = has_alternative_quota_compatible_profile
        && matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        )
        && !matches!(quota_source, Some(RuntimeQuotaSource::LiveProbe));
    let unknown_quota_allowed = quota_summary.route_band == RuntimeQuotaPressureBand::Unknown
        && !has_alternative_quota_compatible_profile;
    let quota_band_blocks_current =
        quota_summary.route_band > RuntimeQuotaPressureBand::Healthy && !unknown_quota_allowed;

    if auth_failure_active
        || in_selection_backoff
        || circuit_open_until.is_some()
        || health_score > 0
        || performance_score > 0
        || quota_evidence_required
        || live_quota_probe_required
        || inflight_count >= inflight_soft_limit
        || quota_band_blocks_current
    {
        let reason = if auth_failure_active {
            "auth_failure_backoff"
        } else if in_selection_backoff {
            "selection_backoff"
        } else if circuit_open_until.is_some() {
            "route_circuit_open"
        } else if health_score > 0 {
            "profile_health"
        } else if performance_score > 0 {
            "profile_performance"
        } else if quota_evidence_required {
            "quota_probe_unavailable"
        } else if live_quota_probe_required {
            if matches!(quota_source, Some(RuntimeQuotaSource::PersistedSnapshot)) {
                "stale_persisted_quota"
            } else {
                "quota_probe_unavailable"
            }
        } else if quota_band_blocks_current {
            runtime_quota_pressure_band_reason(quota_summary.route_band)
        } else {
            "profile_inflight_soft_limit"
        };
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason={} inflight={} health={} performance={} soft_limit={} circuit_until={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                current_profile,
                reason,
                inflight_count,
                health_score,
                performance_score,
                inflight_soft_limit,
                circuit_open_until.unwrap_or_default(),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }
    if !current_profile_quota_compatible {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=auth_not_quota_compatible",
                runtime_route_kind_label(route_kind),
                current_profile
            ),
        );
        return Ok(None);
    }

    runtime_proxy_log(
        shared,
        format!(
            "selection_keep_current route={} profile={} inflight={} health={} performance={} quota_source={} {}",
            runtime_route_kind_label(route_kind),
            current_profile,
            inflight_count,
            health_score,
            performance_score,
            quota_source
                .map(runtime_quota_source_label)
                .unwrap_or("unknown"),
            runtime_quota_summary_log_fields(quota_summary),
        ),
    );
    if !reserve_runtime_profile_route_circuit_half_open_probe(shared, &current_profile, route_kind)?
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} performance={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                current_profile,
                inflight_count,
                health_score,
                performance_score,
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }
    Ok(Some(current_profile))
}
