use super::*;

mod admission;
mod affinity;
mod buffered_response;
mod classification;
mod continuation;
mod dispatch;
mod health;
mod lifecycle;
mod lineage;
mod path;
mod payload_detection;
mod prefetch;
mod profile_state;
mod quota;
mod response_forwarding;
mod standard;
mod transport_failure;
mod upstream;
mod websocket;

pub(crate) use self::admission::*;
pub(crate) use self::affinity::*;
pub(super) use self::buffered_response::*;
pub(crate) use self::classification::*;
pub(super) use self::continuation::*;
pub(crate) use self::dispatch::*;
pub(super) use self::health::*;
pub(crate) use self::lifecycle::*;
pub(super) use self::lineage::*;
pub(super) use self::path::*;
pub(super) use self::payload_detection::*;
pub(super) use self::prefetch::*;
pub(super) use self::profile_state::*;
pub(super) use self::quota::*;
pub(crate) use self::response_forwarding::*;
pub(crate) use self::standard::*;
pub(super) use self::transport_failure::*;
pub(crate) use self::upstream::*;
pub(crate) use self::websocket::*;

pub(super) fn runtime_previous_response_retry_delay(retry_index: usize) -> Option<Duration> {
    RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS
        .get(retry_index)
        .copied()
        .map(Duration::from_millis)
}

pub(super) fn runtime_proxy_precommit_budget_exhausted(
    started_at: Instant,
    attempts: usize,
    continuation: bool,
    pressure_mode: bool,
) -> bool {
    let (attempt_limit, budget) = runtime_proxy_precommit_budget(continuation, pressure_mode);

    attempts >= attempt_limit || started_at.elapsed() >= budget
}

pub(super) fn runtime_proxy_final_retryable_http_failure_response(
    last_failure: Option<(tiny_http::ResponseBox, bool)>,
    saw_inflight_saturation: bool,
    json_errors: bool,
) -> Option<tiny_http::ResponseBox> {
    let service_unavailable = |message: &str| {
        if json_errors {
            build_runtime_proxy_json_error_response(503, "service_unavailable", message)
        } else {
            build_runtime_proxy_text_response(503, message)
        }
    };
    match last_failure {
        Some((response, false)) => Some(response),
        Some((_response, true)) if saw_inflight_saturation => Some(service_unavailable(
            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
        )),
        Some((_response, true)) => Some(service_unavailable(
            runtime_proxy_local_selection_failure_message(),
        )),
        None if saw_inflight_saturation => Some(service_unavailable(
            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
        )),
        None => None,
    }
}

pub(super) fn runtime_proxy_final_responses_failure_reply(
    last_failure: Option<(RuntimeUpstreamFailureResponse, bool)>,
    saw_inflight_saturation: bool,
) -> RuntimeResponsesReply {
    match last_failure {
        Some((failure, false)) => match failure {
            RuntimeUpstreamFailureResponse::Http(response) => response,
            RuntimeUpstreamFailureResponse::Websocket(_) => {
                RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                ))
            }
        },
        _ if saw_inflight_saturation => {
            RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                503,
                "service_unavailable",
                "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
            ))
        }
        _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
            503,
            "service_unavailable",
            runtime_proxy_local_selection_failure_message(),
        )),
    }
}

pub(super) fn send_runtime_proxy_final_websocket_failure(
    local_socket: &mut RuntimeLocalWebSocket,
    last_failure: Option<(RuntimeUpstreamFailureResponse, bool)>,
    saw_inflight_saturation: bool,
) -> Result<()> {
    match last_failure {
        Some((failure, false)) => match failure {
            RuntimeUpstreamFailureResponse::Websocket(payload) => {
                forward_runtime_proxy_websocket_error(local_socket, &payload)
            }
            RuntimeUpstreamFailureResponse::Http(_) => send_runtime_proxy_websocket_error(
                local_socket,
                503,
                "service_unavailable",
                runtime_proxy_local_selection_failure_message(),
            ),
        },
        _ if saw_inflight_saturation => send_runtime_proxy_websocket_error(
            local_socket,
            503,
            "service_unavailable",
            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
        ),
        _ => send_runtime_proxy_websocket_error(
            local_socket,
            503,
            "service_unavailable",
            runtime_proxy_local_selection_failure_message(),
        ),
    }
}

pub(super) fn runtime_proxy_precommit_budget(
    continuation: bool,
    pressure_mode: bool,
) -> (usize, Duration) {
    if continuation {
        (
            RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS),
        )
    } else if pressure_mode {
        (
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS),
        )
    } else {
        (
            RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_BUDGET_MS),
        )
    }
}

pub(super) fn runtime_proxy_has_continuation_priority(
    previous_response_id: Option<&str>,
    pinned_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
) -> bool {
    previous_response_id.is_some()
        || pinned_profile.is_some()
        || request_turn_state.is_some()
        || turn_state_profile.is_some()
        || session_profile.is_some()
}

pub(super) fn runtime_wait_affinity_owner<'a>(
    strict_affinity_profile: Option<&'a str>,
    pinned_profile: Option<&'a str>,
    turn_state_profile: Option<&'a str>,
    session_profile: Option<&'a str>,
    trusted_previous_response_affinity: bool,
) -> Option<&'a str> {
    strict_affinity_profile
        .or(turn_state_profile)
        .or_else(|| {
            trusted_previous_response_affinity
                .then_some(pinned_profile)
                .flatten()
        })
        .or(session_profile)
}

pub(super) fn runtime_noncompact_session_priority_profile<'a>(
    session_profile: Option<&'a str>,
    compact_session_profile: Option<&str>,
) -> Option<&'a str> {
    if compact_session_profile.is_some_and(|profile_name| session_profile == Some(profile_name)) {
        None
    } else {
        session_profile
    }
}

pub(super) fn runtime_proxy_allows_direct_current_profile_fallback(
    previous_response_id: Option<&str>,
    pinned_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    saw_inflight_saturation: bool,
    saw_upstream_failure: bool,
) -> bool {
    previous_response_id.is_none()
        && pinned_profile.is_none()
        && request_turn_state.is_none()
        && turn_state_profile.is_none()
        && session_profile.is_none()
        && !saw_inflight_saturation
        && !saw_upstream_failure
}

pub(super) fn runtime_profile_codex_home(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<Option<PathBuf>> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime
        .state
        .profiles
        .get(profile_name)
        .map(|profile| profile.codex_home.clone()))
}

#[cfg(test)]
pub(super) fn runtime_has_alternative_quota_compatible_profile(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<bool> {
    let allow_disk_fallback = !runtime_proxy_sync_probe_pressure_mode_active_for_route(
        shared,
        RuntimeRouteKind::Responses,
    );
    let fallback_profiles = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let mut fallback_profiles = Vec::new();
        for (name, profile) in &runtime.state.profiles {
            if name == profile_name {
                continue;
            }
            if runtime_profile_cached_auth_summary_for_selection(
                runtime.profile_usage_auth.get(name).cloned(),
                runtime.profile_probe_cache.get(name).cloned(),
            )
            .is_some_and(|summary| summary.quota_compatible)
            {
                return Ok(true);
            }
            fallback_profiles.push((profile.codex_home.clone(), profile.provider.clone()));
        }
        fallback_profiles
    };
    if !allow_disk_fallback {
        return Ok(false);
    }
    Ok(fallback_profiles
        .into_iter()
        .any(|(codex_home, provider)| provider.auth_summary(&codex_home).quota_compatible))
}

pub(super) fn runtime_has_route_eligible_quota_fallback(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let (
        state,
        retry_backoff_until,
        transport_backoff_until,
        route_circuit_open_until,
        profile_inflight,
        profile_health,
        profile_usage_auth,
        profile_probe_cache,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_probe_cache.clone(),
        )
    };
    for (name, profile) in &state.profiles {
        if name == profile_name || excluded_profiles.contains(name) {
            continue;
        }
        if !runtime_profile_auth_summary_for_selection_with_policy(
            name,
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
            name,
            now,
        ) {
            continue;
        }
        if runtime_profile_name_in_selection_backoff(
            name,
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            route_kind,
            now,
        ) {
            continue;
        }
        if runtime_profile_inflight_sort_key(name, &profile_inflight) >= inflight_soft_limit {
            continue;
        }
        return Ok(true);
    }
    Ok(false)
}

pub(super) fn refresh_runtime_profile_quota_inline(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &str,
) -> Result<()> {
    let Some(codex_home) = runtime_profile_codex_home(shared, profile_name)? else {
        return Ok(());
    };
    runtime_proxy_log(shared, format!("{context}_start profile={profile_name}"));
    run_runtime_probe_jobs_inline(
        shared,
        vec![(profile_name.to_string(), codex_home)],
        context,
    );
    Ok(())
}

pub(super) fn runtime_quota_summary_requires_precommit_live_probe(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && !matches!(source, Some(RuntimeQuotaSource::LiveProbe))
        && (matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Critical)
            || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Critical)
            || matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
            || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown))
}

pub(super) fn runtime_quota_summary_requires_live_source_after_probe(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && !matches!(source, Some(RuntimeQuotaSource::LiveProbe))
        && (matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
            || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown))
}

pub(super) fn ensure_runtime_profile_precommit_quota_ready(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
) -> Result<(RuntimeQuotaSummary, Option<RuntimeQuotaSource>)> {
    let (mut quota_summary, mut quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
    if runtime_quota_summary_requires_precommit_live_probe(quota_summary, quota_source, route_kind)
    {
        refresh_runtime_profile_quota_inline(shared, profile_name, context)?;
        (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
    }
    Ok((quota_summary, quota_source))
}

pub(super) fn runtime_proxy_direct_current_fallback_profile(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let (profile_name, codex_home, auth_failure_active, cached_usage_auth_entry, probe_cache_entry) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let profile_name = runtime.current_profile.clone();
        let Some(profile) = runtime.state.profiles.get(&profile_name) else {
            return Ok(None);
        };
        let now = Local::now().timestamp();
        (
            profile_name.clone(),
            profile.codex_home.clone(),
            runtime_profile_auth_failure_active(&runtime, &profile_name, now),
            runtime.profile_usage_auth.get(&profile_name).cloned(),
            runtime.profile_probe_cache.get(&profile_name).cloned(),
        )
    };
    if excluded_profiles.contains(&profile_name) {
        return Ok(None);
    }
    if auth_failure_active {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=auth_failure_backoff",
                runtime_route_kind_label(route_kind),
                profile_name,
            ),
        );
        return Ok(None);
    }
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    if !runtime_profile_cached_auth_summary_for_selection(
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
    .quota_compatible
    {
        return Ok(None);
    }
    if runtime_profile_inflight_hard_limited_for_context(
        shared,
        &profile_name,
        runtime_route_kind_inflight_context(route_kind),
    )? {
        return Ok(None);
    }
    if matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) {
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, &profile_name, route_kind)?;
        if quota_source.is_none()
            || runtime_quota_summary_requires_live_source_after_probe(
                quota_summary,
                quota_source,
                route_kind,
            )
            || runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some()
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    profile_name,
                    runtime_quota_precommit_guard_reason(quota_summary, route_kind).unwrap_or_else(
                        || {
                            runtime_quota_soft_affinity_rejection_reason(
                                quota_summary,
                                quota_source,
                                route_kind,
                            )
                        }
                    ),
                    quota_source
                        .map(runtime_quota_source_label)
                        .unwrap_or("unknown"),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            return Ok(None);
        }
    }
    Ok(Some(profile_name))
}

pub(super) fn runtime_proxy_local_selection_failure_message() -> &'static str {
    "Runtime proxy could not secure a healthy upstream profile before the pre-commit retry budget was exhausted. Retry the request."
}

pub(super) fn runtime_quota_window_usable_for_auto_rotate(
    status: RuntimeQuotaWindowStatus,
) -> bool {
    matches!(
        status,
        RuntimeQuotaWindowStatus::Ready
            | RuntimeQuotaWindowStatus::Thin
            | RuntimeQuotaWindowStatus::Critical
    )
}

pub(super) fn runtime_quota_summary_allows_soft_affinity(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    source.is_some()
        && runtime_quota_window_usable_for_auto_rotate(summary.five_hour.status)
        && runtime_quota_window_usable_for_auto_rotate(summary.weekly.status)
        && runtime_quota_precommit_guard_reason(summary, route_kind).is_none()
}

pub(super) fn runtime_quota_soft_affinity_rejection_reason(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> &'static str {
    if source.is_none()
        || matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
        || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown)
    {
        "quota_windows_unavailable"
    } else if let Some(reason) = runtime_quota_precommit_guard_reason(summary, route_kind) {
        reason
    } else if matches!(
        summary.five_hour.status,
        RuntimeQuotaWindowStatus::Exhausted
    ) || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Exhausted)
    {
        "quota_exhausted"
    } else {
        runtime_quota_pressure_band_reason(summary.route_band)
    }
}

pub(super) fn runtime_quota_source_sort_key(
    route_kind: RuntimeRouteKind,
    source: RuntimeQuotaSource,
) -> usize {
    match (route_kind, source) {
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeQuotaSource::LiveProbe,
        ) => 0,
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeQuotaSource::PersistedSnapshot,
        ) => 1,
        _ => 0,
    }
}

pub(super) fn runtime_profile_quota_summary_for_route(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<(RuntimeQuotaSummary, Option<RuntimeQuotaSource>)> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    Ok(runtime
        .profile_probe_cache
        .get(profile_name)
        .filter(|entry| runtime_profile_usage_cache_is_fresh(entry, now))
        .and_then(|entry| entry.result.as_ref().ok())
        .map(|usage| {
            (
                runtime_quota_summary_for_route(usage, route_kind),
                Some(RuntimeQuotaSource::LiveProbe),
            )
        })
        .or_else(|| {
            runtime
                .profile_usage_snapshots
                .get(profile_name)
                .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
                .map(|snapshot| {
                    (
                        runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now),
                        Some(RuntimeQuotaSource::PersistedSnapshot),
                    )
                })
        })
        .unwrap_or((
            RuntimeQuotaSummary {
                five_hour: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                weekly: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                route_band: RuntimeQuotaPressureBand::Unknown,
            },
            None,
        )))
}

pub(super) fn runtime_profile_cached_auth_summary_for_selection(
    usage_auth_entry: Option<RuntimeProfileUsageAuthCacheEntry>,
    probe_entry: Option<RuntimeProfileProbeCacheEntry>,
) -> Option<AuthSummary> {
    if let Some(entry) = usage_auth_entry {
        match runtime_profile_usage_auth_cache_entry_freshness(&entry) {
            RuntimeProfileUsageAuthCacheFreshness::Fresh => {
                return Some(AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                });
            }
            RuntimeProfileUsageAuthCacheFreshness::Stale
            | RuntimeProfileUsageAuthCacheFreshness::Unknown => {}
        }
    }
    probe_entry.map(|entry| entry.auth)
}

pub(super) fn runtime_profile_cached_auth_summary_from_maps_for_selection(
    profile_name: &str,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
) -> Option<AuthSummary> {
    runtime_profile_cached_auth_summary_for_selection(
        profile_usage_auth.get(profile_name).cloned(),
        profile_probe_cache.get(profile_name).cloned(),
    )
}

pub(super) fn runtime_snapshot_blocks_same_request_cold_start_probe(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && runtime_usage_snapshot_is_usable(snapshot, now)
        && runtime_quota_precommit_guard_reason(
            runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now),
            route_kind,
        )
        .is_some()
}

#[cfg(test)]
pub(super) fn runtime_profile_auth_summary_for_selection(
    profile_name: &str,
    codex_home: &Path,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
) -> AuthSummary {
    runtime_profile_auth_summary_for_selection_with_policy(
        profile_name,
        codex_home,
        profile_usage_auth,
        profile_probe_cache,
        true,
    )
    .unwrap_or_else(runtime_profile_uncached_auth_summary_for_selection)
}

pub(super) fn runtime_profile_uncached_auth_summary_for_selection() -> AuthSummary {
    AuthSummary {
        label: "uncached-auth".to_string(),
        quota_compatible: false,
    }
}

pub(super) fn runtime_profile_auth_summary_for_selection_with_policy(
    profile_name: &str,
    codex_home: &Path,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    allow_disk_fallback: bool,
) -> Option<AuthSummary> {
    runtime_profile_cached_auth_summary_from_maps_for_selection(
        profile_name,
        profile_usage_auth,
        profile_probe_cache,
    )
    .or_else(|| allow_disk_fallback.then(|| read_auth_summary(codex_home)))
}

pub(super) fn runtime_proxy_sync_probe_pressure_pause(
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

pub(super) fn runtime_previous_response_affinity_is_trusted(
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

pub(super) fn runtime_previous_response_affinity_is_bound(
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
pub(super) struct RuntimeCandidateAffinity<'a> {
    route_kind: RuntimeRouteKind,
    candidate_name: &'a str,
    strict_affinity_profile: Option<&'a str>,
    pinned_profile: Option<&'a str>,
    turn_state_profile: Option<&'a str>,
    session_profile: Option<&'a str>,
    trusted_previous_response_affinity: bool,
}

impl<'a> RuntimeCandidateAffinity<'a> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn new(
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

pub(super) fn runtime_candidate_has_hard_affinity(affinity: RuntimeCandidateAffinity<'_>) -> bool {
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

pub(super) fn runtime_quota_blocked_affinity_is_releasable(
    affinity: RuntimeCandidateAffinity<'_>,
    request_requires_previous_response_affinity: bool,
) -> bool {
    if request_requires_previous_response_affinity {
        // Some continuations cannot be replayed safely on another account/session. Tool outputs
        // carry chain-scoped call ids, and websocket previous_response continuations without turn
        // state would silently degrade into fresh requests. These requests must fail in place.
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
        // Pre-commit quota or overload means the owning response chain is temporarily unavailable,
        // so a fresh fallback may safely drop previous_response affinity even for *_call_output.
        return true;
    }

    true
}

pub(super) fn runtime_quota_blocked_previous_response_fresh_fallback_allowed(
    previous_response_id: Option<&str>,
    trusted_previous_response_affinity: bool,
    previous_response_fresh_fallback_used: bool,
) -> bool {
    previous_response_id.is_some()
        && trusted_previous_response_affinity
        && !previous_response_fresh_fallback_used
}

pub(super) fn runtime_quota_precommit_floor_percent(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => {
            runtime_proxy_responses_quota_critical_floor_percent()
        }
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 1,
    }
}

pub(super) fn runtime_quota_window_precommit_guard(
    window: RuntimeQuotaWindowSummary,
    floor_percent: i64,
) -> bool {
    matches!(
        window.status,
        RuntimeQuotaWindowStatus::Critical | RuntimeQuotaWindowStatus::Exhausted
    ) && window.remaining_percent <= floor_percent
}

pub(super) fn runtime_quota_precommit_guard_reason(
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
pub(super) struct RuntimeResponseCandidateSelection<'a> {
    excluded_profiles: &'a BTreeSet<String>,
    strict_affinity_profile: Option<&'a str>,
    pinned_profile: Option<&'a str>,
    turn_state_profile: Option<&'a str>,
    session_profile: Option<&'a str>,
    discover_previous_response_owner: bool,
    previous_response_id: Option<&'a str>,
    route_kind: RuntimeRouteKind,
}

#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_response_candidate_for_route(
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

pub(super) fn select_runtime_response_candidate_for_route_with_selection(
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

pub(super) fn next_runtime_previous_response_candidate(
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

pub(super) fn runtime_proxy_optimistic_current_candidate_for_route(
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

#[allow(clippy::too_many_arguments)]
pub(super) fn proxy_runtime_websocket_text_message(
    session_id: u64,
    request_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    request_text: &str,
    request_metadata: &RuntimeWebsocketRequestMetadata,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
) -> Result<()> {
    let mut handshake_request = handshake_request.clone();
    let mut request_text = request_text.to_string();
    let request_requires_previous_response_affinity =
        request_metadata.requires_previous_response_affinity;
    let mut previous_response_id = request_metadata.previous_response_id.clone();
    let mut request_turn_state = runtime_request_turn_state(&handshake_request);
    let explicit_request_session_id = runtime_request_explicit_session_id(&handshake_request);
    let request_session_id_header_present = explicit_request_session_id.is_some();
    let request_session_id = explicit_request_session_id
        .as_ref()
        .map(|session_id| session_id.as_str().to_string())
        .or_else(|| request_metadata.session_id.clone());
    let mut bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Websocket)
        })
        .transpose()?
        .flatten();
    let mut trusted_previous_response_affinity = runtime_previous_response_affinity_is_trusted(
        shared,
        previous_response_id.as_deref(),
        bound_profile.as_deref(),
    )?;
    if request_turn_state.is_none()
        && let Some(turn_state) = runtime_previous_response_turn_state(
            shared,
            previous_response_id.as_deref(),
            bound_profile.as_deref(),
        )?
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket route=responses previous_response_turn_state_rehydrated response_id={} profile={} turn_state={turn_state}",
                previous_response_id.as_deref().unwrap_or("-"),
                bound_profile.as_deref().unwrap_or("-"),
            ),
        );
        request_turn_state = Some(turn_state);
    }
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut bound_session_profile = None;
    let mut compact_followup_profile = None;
    let mut compact_session_profile = None;
    let mut session_profile = None;
    let mut pinned_profile = None;
    macro_rules! recompute_route_affinity {
        ($reason:expr) => {
            refresh_and_log_runtime_response_route_affinity(
                shared,
                request_id,
                Some(session_id),
                $reason,
                previous_response_id.as_deref(),
                bound_profile.as_deref(),
                turn_state_profile.as_deref(),
                request_turn_state.as_deref(),
                request_session_id.as_deref(),
                explicit_request_session_id.as_ref(),
                websocket_session.profile_name.as_deref(),
                &mut bound_session_profile,
                &mut compact_followup_profile,
                &mut compact_session_profile,
                &mut session_profile,
                &mut pinned_profile,
            )
        };
    }
    recompute_route_affinity!("initial")?;
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure: Option<(RuntimeUpstreamFailureResponse, bool)> = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;
    let mut saw_inflight_saturation = false;
    let mut selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    let mut previous_response_fresh_fallback_used = false;
    let mut saw_previous_response_not_found = false;
    let mut websocket_reuse_fresh_retry_profiles = BTreeSet::new();
    macro_rules! runtime_candidate_affinity {
        ($route_kind:expr, $candidate_name:expr, $strict_affinity_profile:expr $(,)?) => {
            RuntimeCandidateAffinity {
                route_kind: $route_kind,
                candidate_name: $candidate_name,
                strict_affinity_profile: $strict_affinity_profile,
                pinned_profile: pinned_profile.as_deref(),
                turn_state_profile: turn_state_profile.as_deref(),
                session_profile: session_profile.as_deref(),
                trusted_previous_response_affinity,
            }
        };
    }
    macro_rules! runtime_websocket_fresh_fallback_target {
        () => {
            RuntimeWebsocketFreshFallbackTarget {
                request_text: &mut request_text,
                handshake_request: &mut handshake_request,
                websocket_reuse_fresh_retry_profiles: &mut websocket_reuse_fresh_retry_profiles,
            }
        };
    }
    macro_rules! runtime_previous_response_fresh_fallback_state {
        () => {
            RuntimePreviousResponseFreshFallbackState {
                previous_response_id: &mut previous_response_id,
                request_turn_state: &mut request_turn_state,
                previous_response_fresh_fallback_used: &mut previous_response_fresh_fallback_used,
                saw_previous_response_not_found: &mut saw_previous_response_not_found,
                previous_response_retry_candidate: &mut previous_response_retry_candidate,
                previous_response_retry_index: &mut previous_response_retry_index,
                candidate_turn_state_retry_profile: &mut candidate_turn_state_retry_profile,
                candidate_turn_state_retry_value: &mut candidate_turn_state_retry_value,
                trusted_previous_response_affinity: &mut trusted_previous_response_affinity,
                bound_profile: &mut bound_profile,
                pinned_profile: &mut pinned_profile,
                turn_state_profile: &mut turn_state_profile,
                session_profile: &mut session_profile,
                excluded_profiles: &mut excluded_profiles,
                last_failure: &mut last_failure,
                selection_started_at: &mut selection_started_at,
                selection_attempts: &mut selection_attempts,
            }
        };
    }

    loop {
        let pressure_mode =
            runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Websocket);
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            runtime_proxy_has_continuation_priority(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
            ),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_websocket_request_requires_locked_previous_response_affinity(
                    request_requires_previous_response_affinity,
                    trusted_previous_response_affinity,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                )
                && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=precommit_budget_exhausted"
                    ),
                );
                apply_runtime_websocket_previous_response_fresh_fallback(
                    fresh_request_text,
                    runtime_websocket_fresh_fallback_target!(),
                    runtime_previous_response_fresh_fallback_state!(),
                );
                recompute_route_affinity!("previous_response_fresh_fallback")?;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} compact_fresh_fallback_blocked profile={profile_name} source={source} reason=precommit_budget_exhausted"
                    ),
                );
                send_runtime_proxy_final_websocket_failure(
                    local_socket,
                    last_failure,
                    saw_inflight_saturation,
                )?;
                return Ok(());
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Websocket,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} direct_current_profile_fallback profile={current_profile} reason=precommit_budget_exhausted"
                    ),
                );
                match attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
                    request_id,
                    local_socket,
                    handshake_request: &handshake_request,
                    request_text: &request_text,
                    request_previous_response_id: previous_response_id.as_deref(),
                    request_session_id: request_session_id.as_deref(),
                    request_turn_state: request_turn_state.as_deref(),
                    shared,
                    websocket_session,
                    profile_name: &current_profile,
                    turn_state_override: request_turn_state.as_deref(),
                    promote_committed_profile: previous_response_id.is_none()
                        && bound_profile.is_none()
                        && request_turn_state.is_none()
                        && turn_state_profile.is_none()
                        && compact_followup_profile.is_none()
                        && !(request_session_id_header_present || bound_session_profile.is_some()),
                })? {
                    RuntimeWebsocketAttempt::Delivered => return Ok(()),
                    RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name,
                        payload,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request_text) =
                            runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_websocket_previous_response_fresh_fallback(
                                fresh_request_text,
                                runtime_websocket_fresh_fallback_target!(),
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        if !runtime_has_route_eligible_quota_fallback(
                            shared,
                            &profile_name,
                            &BTreeSet::new(),
                            RuntimeRouteKind::Websocket,
                        )? {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), true));
                        continue;
                    }
                    RuntimeWebsocketAttempt::Overloaded {
                        profile_name,
                        payload,
                    } => {
                        let overload_message =
                            extract_runtime_proxy_overload_message_from_websocket_payload(&payload);
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} upstream_overloaded route=websocket profile={profile_name} via=direct_current_profile_fallback message={}",
                                overload_message.as_deref().unwrap_or("-"),
                            ),
                        );
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        let _ = bump_runtime_profile_health_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                            "websocket_overload",
                        );
                        let _ = bump_runtime_profile_bad_pairing_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                            "websocket_overload",
                        );
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            ),
                        ) {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} upstream_overload_passthrough route=websocket profile={profile_name} reason=hard_affinity via=direct_current_profile_fallback"
                                ),
                            );
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request_text) =
                            runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=upstream_overloaded via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_websocket_previous_response_fresh_fallback(
                                fresh_request_text,
                                runtime_websocket_fresh_fallback_target!(),
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                        continue;
                    }
                    RuntimeWebsocketAttempt::PreviousResponseNotFound {
                        profile_name,
                        payload,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure =
                                Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry
                            && !runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            )
                        {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Websocket,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            Some(&mut compact_followup_profile),
                        );
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                        continue;
                    }
                    RuntimeWebsocketAttempt::ReuseWatchdogTripped { profile_name, .. } => {
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                    RuntimeWebsocketAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            ),
                        ) {
                            send_runtime_proxy_websocket_error(
                                local_socket,
                                503,
                                "service_unavailable",
                                runtime_proxy_local_selection_failure_message(),
                            )?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request_text) =
                            runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_websocket_previous_response_fresh_fallback(
                                fresh_request_text,
                                runtime_websocket_fresh_fallback_target!(),
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            send_runtime_proxy_final_websocket_failure(
                local_socket,
                last_failure,
                saw_inflight_saturation,
            )?;
            return Ok(());
        }

        let Some(candidate_name) = select_runtime_response_candidate_for_route_with_selection(
            shared,
            RuntimeResponseCandidateSelection {
                excluded_profiles: &excluded_profiles,
                strict_affinity_profile: compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                pinned_profile: pinned_profile.as_deref(),
                turn_state_profile: turn_state_profile.as_deref(),
                session_profile: session_profile.as_deref(),
                discover_previous_response_owner: previous_response_id.is_some(),
                previous_response_id: previous_response_id.as_deref(),
                route_kind: RuntimeRouteKind::Websocket,
            },
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} candidate_exhausted last_failure={}",
                    match &last_failure {
                        Some((RuntimeUpstreamFailureResponse::Websocket(_), _)) => "websocket",
                        Some((RuntimeUpstreamFailureResponse::Http(_), _)) => "http",
                        None => "none",
                    }
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_websocket_request_requires_locked_previous_response_affinity(
                    request_requires_previous_response_affinity,
                    trusted_previous_response_affinity,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                )
                && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=candidate_exhausted"
                    ),
                );
                apply_runtime_websocket_previous_response_fresh_fallback(
                    fresh_request_text,
                    runtime_websocket_fresh_fallback_target!(),
                    runtime_previous_response_fresh_fallback_state!(),
                );
                recompute_route_affinity!("previous_response_fresh_fallback")?;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} compact_fresh_fallback_blocked profile={profile_name} source={source} reason=candidate_exhausted"
                    ),
                );
                send_runtime_proxy_final_websocket_failure(
                    local_socket,
                    last_failure,
                    saw_inflight_saturation,
                )?;
                return Ok(());
            }
            let remaining_cold_start_profiles =
                runtime_remaining_sync_probe_cold_start_profiles_for_route(
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Websocket,
                )?;
            if remaining_cold_start_profiles > 0 {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} candidate_exhausted_continue route=websocket remaining_cold_start_profiles={remaining_cold_start_profiles}"
                    ),
                );
                runtime_proxy_sync_probe_pressure_pause(shared, RuntimeRouteKind::Websocket);
                continue;
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Websocket,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} direct_current_profile_fallback profile={current_profile} reason=candidate_exhausted"
                    ),
                );
                match attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
                    request_id,
                    local_socket,
                    handshake_request: &handshake_request,
                    request_text: &request_text,
                    request_previous_response_id: previous_response_id.as_deref(),
                    request_session_id: request_session_id.as_deref(),
                    request_turn_state: request_turn_state.as_deref(),
                    shared,
                    websocket_session,
                    profile_name: &current_profile,
                    turn_state_override: request_turn_state.as_deref(),
                    promote_committed_profile: previous_response_id.is_none()
                        && bound_profile.is_none()
                        && request_turn_state.is_none()
                        && turn_state_profile.is_none()
                        && compact_followup_profile.is_none()
                        && !(request_session_id_header_present || bound_session_profile.is_some()),
                })? {
                    RuntimeWebsocketAttempt::Delivered => return Ok(()),
                    RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name,
                        payload,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request_text) =
                            runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_websocket_previous_response_fresh_fallback(
                                fresh_request_text,
                                runtime_websocket_fresh_fallback_target!(),
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        if !runtime_has_route_eligible_quota_fallback(
                            shared,
                            &profile_name,
                            &BTreeSet::new(),
                            RuntimeRouteKind::Websocket,
                        )? {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), true));
                        continue;
                    }
                    RuntimeWebsocketAttempt::Overloaded {
                        profile_name,
                        payload,
                    } => {
                        let overload_message =
                            extract_runtime_proxy_overload_message_from_websocket_payload(&payload);
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} upstream_overloaded route=websocket profile={profile_name} via=direct_current_profile_fallback message={}",
                                overload_message.as_deref().unwrap_or("-"),
                            ),
                        );
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        let _ = bump_runtime_profile_health_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                            "websocket_overload",
                        );
                        let _ = bump_runtime_profile_bad_pairing_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                            "websocket_overload",
                        );
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            ),
                        ) {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} upstream_overload_passthrough route=websocket profile={profile_name} reason=hard_affinity via=direct_current_profile_fallback"
                                ),
                            );
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                        continue;
                    }
                    RuntimeWebsocketAttempt::PreviousResponseNotFound {
                        profile_name,
                        payload,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure =
                                Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry
                            && !runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            )
                        {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Websocket,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            false,
                            &mut turn_state_profile,
                            Some(&mut compact_followup_profile),
                        );
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                        continue;
                    }
                    RuntimeWebsocketAttempt::ReuseWatchdogTripped { profile_name, .. } => {
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                    RuntimeWebsocketAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            ),
                        ) {
                            send_runtime_proxy_websocket_error(
                                local_socket,
                                503,
                                "service_unavailable",
                                runtime_proxy_local_selection_failure_message(),
                            )?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            false,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request_text) =
                            runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_websocket_previous_response_fresh_fallback(
                                fresh_request_text,
                                runtime_websocket_fresh_fallback_target!(),
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            send_runtime_proxy_final_websocket_failure(
                local_socket,
                last_failure,
                saw_inflight_saturation,
            )?;
            return Ok(());
        };
        selection_attempts = selection_attempts.saturating_add(1);
        let turn_state_override =
            if candidate_turn_state_retry_profile.as_deref() == Some(candidate_name.as_str()) {
                candidate_turn_state_retry_value.as_deref()
            } else {
                request_turn_state.as_deref()
            };
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} websocket_session={session_id} candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                candidate_name,
                pinned_profile,
                turn_state_profile,
                turn_state_override,
                excluded_profiles.len()
            ),
        );
        let session_affinity_candidate =
            session_profile.as_deref() == Some(candidate_name.as_str());
        if previous_response_id.is_none()
            && pinned_profile.is_none()
            && turn_state_profile.is_none()
            && !session_affinity_candidate
            && runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate_name,
                "websocket_session",
            )?
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} profile_inflight_saturated profile={candidate_name} hard_limit={}",
                    runtime_proxy_profile_inflight_hard_limit(),
                ),
            );
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
            continue;
        }

        match attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
            request_id,
            local_socket,
            handshake_request: &handshake_request,
            request_text: &request_text,
            request_previous_response_id: previous_response_id.as_deref(),
            request_session_id: request_session_id.as_deref(),
            request_turn_state: request_turn_state.as_deref(),
            shared,
            websocket_session,
            profile_name: &candidate_name,
            turn_state_override,
            promote_committed_profile: previous_response_id.is_none()
                && bound_profile.is_none()
                && request_turn_state.is_none()
                && turn_state_profile.is_none()
                && compact_followup_profile.is_none()
                && !(request_session_id_header_present || bound_session_profile.is_some()),
        })? {
            RuntimeWebsocketAttempt::Delivered => return Ok(()),
            RuntimeWebsocketAttempt::QuotaBlocked {
                profile_name,
                payload,
            } => {
                let quota_message =
                    extract_runtime_proxy_quota_message_from_websocket_payload(&payload);
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} quota_blocked profile={profile_name}"
                    ),
                );
                mark_runtime_profile_quota_quarantine(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Websocket,
                    quota_message.as_deref(),
                )?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    runtime_candidate_affinity!(
                        RuntimeRouteKind::Websocket,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                    ),
                    request_requires_previous_response_affinity,
                ) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} upstream_usage_limit_passthrough route=websocket profile={profile_name} reason=hard_affinity"
                        ),
                    );
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    return Ok(());
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name}"
                        ),
                    );
                }
                if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                    previous_response_id.as_deref(),
                    trusted_previous_response_affinity,
                    previous_response_fresh_fallback_used,
                ) && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=quota_blocked"
                        ),
                    );
                    apply_runtime_websocket_previous_response_fresh_fallback(
                        fresh_request_text,
                        runtime_websocket_fresh_fallback_target!(),
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                if !runtime_has_route_eligible_quota_fallback(
                    shared,
                    &profile_name,
                    &BTreeSet::new(),
                    RuntimeRouteKind::Websocket,
                )? {
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    return Ok(());
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), true));
            }
            RuntimeWebsocketAttempt::Overloaded {
                profile_name,
                payload,
            } => {
                let overload_message =
                    extract_runtime_proxy_overload_message_from_websocket_payload(&payload);
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} upstream_overloaded route=websocket profile={profile_name} message={}",
                        overload_message.as_deref().unwrap_or("-"),
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                let _ = bump_runtime_profile_health_score(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Websocket,
                    RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                    "websocket_overload",
                );
                let _ = bump_runtime_profile_bad_pairing_score(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Websocket,
                    RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                    "websocket_overload",
                );
                if !runtime_quota_blocked_affinity_is_releasable(
                    runtime_candidate_affinity!(
                        RuntimeRouteKind::Websocket,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                    ),
                    runtime_websocket_request_requires_locked_previous_response_affinity(
                        request_requires_previous_response_affinity,
                        trusted_previous_response_affinity,
                        previous_response_id.as_deref(),
                        request_turn_state.as_deref(),
                    ),
                ) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} upstream_overload_passthrough route=websocket profile={profile_name} reason=hard_affinity"
                        ),
                    );
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    return Ok(());
                }
                if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                    previous_response_id.as_deref(),
                    trusted_previous_response_affinity,
                    previous_response_fresh_fallback_used,
                ) && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=upstream_overloaded"
                        ),
                    );
                    apply_runtime_websocket_previous_response_fresh_fallback(
                        fresh_request_text,
                        runtime_websocket_fresh_fallback_target!(),
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
            }
            RuntimeWebsocketAttempt::LocalSelectionBlocked {
                profile_name,
                reason,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} local_selection_blocked profile={profile_name} reason={reason}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    runtime_candidate_affinity!(
                        RuntimeRouteKind::Websocket,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                    ),
                    runtime_websocket_request_requires_locked_previous_response_affinity(
                        request_requires_previous_response_affinity,
                        trusted_previous_response_affinity,
                        previous_response_id.as_deref(),
                        request_turn_state.as_deref(),
                    ),
                ) {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    )?;
                    return Ok(());
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} reason={reason}"
                        ),
                    );
                }
                if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                    previous_response_id.as_deref(),
                    trusted_previous_response_affinity,
                    previous_response_fresh_fallback_used,
                ) && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason={reason}"
                        ),
                    );
                    apply_runtime_websocket_previous_response_fresh_fallback(
                        fresh_request_text,
                        runtime_websocket_fresh_fallback_target!(),
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name,
                event,
            } => {
                let reuse_terminal_idle = websocket_session.last_terminal_elapsed();
                let retry_same_profile_with_fresh_connect = !websocket_reuse_fresh_retry_profiles
                    .contains(&profile_name)
                    && (bound_profile.as_deref() == Some(profile_name.as_str())
                        || turn_state_profile.as_deref() == Some(profile_name.as_str())
                        || compact_followup_profile
                            .as_ref()
                            .is_some_and(|(owner, _)| owner == &profile_name)
                        || (request_session_id.is_some()
                            && session_profile.as_deref() == Some(profile_name.as_str())));
                let reuse_failed_bound_previous_response = previous_response_id.is_some()
                    && !previous_response_fresh_fallback_used
                    && (bound_profile.as_deref() == Some(profile_name.as_str())
                        || pinned_profile.as_deref() == Some(profile_name.as_str()));
                let nonreplayable_previous_response_reuse = previous_response_id.is_some()
                    && !previous_response_fresh_fallback_used
                    && turn_state_override.is_none();
                let stale_previous_response_reuse = nonreplayable_previous_response_reuse
                    && turn_state_override.is_none()
                    && reuse_terminal_idle.is_some_and(|elapsed| {
                        elapsed
                            >= Duration::from_millis(
                                runtime_proxy_websocket_previous_response_reuse_stale_ms(),
                            )
                    });
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} websocket_reuse_watchdog_timeout profile={profile_name} event={event}"
                    ),
                );
                if nonreplayable_previous_response_reuse {
                    if stale_previous_response_reuse {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} websocket_reuse_stale_previous_response_blocked profile={profile_name} event={event} elapsed_ms={} threshold_ms={}",
                                reuse_terminal_idle
                                    .map(|elapsed| elapsed.as_millis())
                                    .unwrap_or(0),
                                runtime_proxy_websocket_previous_response_reuse_stale_ms(),
                            ),
                        );
                    } else {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} websocket_reuse_previous_response_blocked profile={profile_name} event={event} reason=missing_turn_state elapsed_ms={}",
                                reuse_terminal_idle
                                    .map(|elapsed| elapsed.as_millis())
                                    .unwrap_or(0),
                            ),
                        );
                    }
                    return Err(anyhow::anyhow!(
                        "runtime websocket upstream closed before response.completed for previous_response_id continuation without replayable turn_state: profile={profile_name} event={event}"
                    ));
                }
                if retry_same_profile_with_fresh_connect {
                    websocket_reuse_fresh_retry_profiles.insert(profile_name.clone());
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} websocket_reuse_owner_fresh_retry profile={profile_name} event={event}"
                        ),
                    );
                    continue;
                }
                if reuse_failed_bound_previous_response
                    && !runtime_websocket_request_requires_locked_previous_response_affinity(
                        request_requires_previous_response_affinity,
                        trusted_previous_response_affinity,
                        previous_response_id.as_deref(),
                        request_turn_state.as_deref(),
                    )
                    && let Some(fresh_request_text) =
                        runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=websocket_reuse_watchdog"
                        ),
                    );
                    apply_runtime_websocket_previous_response_fresh_fallback(
                        fresh_request_text,
                        runtime_websocket_fresh_fallback_target!(),
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
                excluded_profiles.insert(profile_name);
            }
            RuntimeWebsocketAttempt::PreviousResponseNotFound {
                profile_name,
                payload,
                turn_state,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?}",
                        turn_state
                    ),
                );
                saw_previous_response_not_found = true;
                if previous_response_retry_candidate.as_deref() != Some(profile_name.as_str()) {
                    previous_response_retry_candidate = Some(profile_name.clone());
                    previous_response_retry_index = 0;
                }
                let has_turn_state_retry = turn_state.is_some();
                if has_turn_state_retry {
                    candidate_turn_state_retry_profile = Some(profile_name.clone());
                    candidate_turn_state_retry_value = turn_state;
                }
                if has_turn_state_retry
                    && let Some(delay) =
                        runtime_previous_response_retry_delay(previous_response_retry_index)
                {
                    previous_response_retry_index += 1;
                    last_failure =
                        Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry",
                            delay.as_millis()
                        ),
                    );
                    continue;
                }
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                if !has_turn_state_retry
                    && !runtime_websocket_request_requires_locked_previous_response_affinity(
                        request_requires_previous_response_affinity,
                        trusted_previous_response_affinity,
                        previous_response_id.as_deref(),
                        request_turn_state.as_deref(),
                    )
                {
                    let _ = clear_runtime_stale_previous_response_binding(
                        shared,
                        &profile_name,
                        previous_response_id.as_deref(),
                    )?;
                }
                let released_affinity = release_runtime_previous_response_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                    RuntimeRouteKind::Websocket,
                )?;
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name}"
                        ),
                    );
                }
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    false,
                    &mut turn_state_profile,
                    Some(&mut compact_followup_profile),
                );
                trusted_previous_response_affinity = false;
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
            }
        }
    }
}

pub(super) struct RuntimeWebsocketAttemptRequest<'a> {
    request_id: u64,
    local_socket: &'a mut RuntimeLocalWebSocket,
    handshake_request: &'a RuntimeProxyRequest,
    request_text: &'a str,
    request_previous_response_id: Option<&'a str>,
    request_session_id: Option<&'a str>,
    request_turn_state: Option<&'a str>,
    shared: &'a RuntimeRotationProxyShared,
    websocket_session: &'a mut RuntimeWebsocketSessionState,
    profile_name: &'a str,
    turn_state_override: Option<&'a str>,
    promote_committed_profile: bool,
}

pub(super) fn attempt_runtime_websocket_request(
    attempt: RuntimeWebsocketAttemptRequest<'_>,
) -> Result<RuntimeWebsocketAttempt> {
    let RuntimeWebsocketAttemptRequest {
        request_id,
        local_socket,
        handshake_request,
        request_text,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
        shared,
        websocket_session,
        profile_name,
        turn_state_override,
        promote_committed_profile,
    } = attempt;

    let realtime_websocket = is_runtime_realtime_websocket_path(&handshake_request.path_and_query);
    let (initial_quota_summary, initial_quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Websocket)?;
    if (request_previous_response_id.is_some()
        || request_session_id.is_some()
        || request_turn_state.is_some())
        && matches!(
            initial_quota_source,
            Some(RuntimeQuotaSource::PersistedSnapshot)
        )
        && let Some(reason) =
            runtime_quota_precommit_guard_reason(initial_quota_summary, RuntimeRouteKind::Websocket)
    {
        websocket_session.close();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket websocket_pre_send_skip profile={profile_name} reason={reason} quota_source={} {}",
                initial_quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(initial_quota_summary),
            ),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }
    let has_alternative_quota_profile = runtime_has_route_eligible_quota_fallback(
        shared,
        profile_name,
        &BTreeSet::new(),
        RuntimeRouteKind::Websocket,
    )?;
    let (quota_summary, quota_source) = ensure_runtime_profile_precommit_quota_ready(
        shared,
        profile_name,
        RuntimeRouteKind::Websocket,
        "websocket_precommit_reprobe",
    )?;
    if runtime_quota_summary_requires_live_source_after_probe(
        quota_summary,
        quota_source,
        RuntimeRouteKind::Websocket,
    ) && has_alternative_quota_profile
    {
        websocket_session.close();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket websocket_pre_send_skip profile={profile_name} reason=quota_windows_unavailable_after_reprobe quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: "quota_windows_unavailable_after_reprobe",
        });
    }
    if let Some(reason) =
        runtime_quota_precommit_guard_reason(quota_summary, RuntimeRouteKind::Websocket)
    {
        websocket_session.close();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket websocket_pre_send_skip profile={profile_name} reason={reason} quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }

    let reuse_existing_session = websocket_session.can_reuse(profile_name, turn_state_override);
    let reuse_started_at = reuse_existing_session.then(Instant::now);
    let precommit_started_at = Instant::now();
    let (mut upstream_socket, mut upstream_turn_state, mut inflight_guard) =
        if reuse_existing_session {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket websocket_reuse_start profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=reuse profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            (
                websocket_session
                    .take_socket()
                    .expect("runtime websocket session should keep its upstream socket"),
                websocket_session.turn_state.clone(),
                None,
            )
        } else {
            websocket_session.close();
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=connect profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            match connect_runtime_proxy_upstream_websocket(
                request_id,
                handshake_request,
                shared,
                profile_name,
                turn_state_override,
            )? {
                RuntimeWebsocketConnectResult::Connected { socket, turn_state } => (
                    socket,
                    turn_state,
                    Some(acquire_runtime_profile_inflight_guard(
                        shared,
                        profile_name,
                        "websocket_session",
                    )?),
                ),
                RuntimeWebsocketConnectResult::QuotaBlocked(payload) => {
                    return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name: profile_name.to_string(),
                        payload,
                    });
                }
                RuntimeWebsocketConnectResult::Overloaded(payload) => {
                    return Ok(RuntimeWebsocketAttempt::Overloaded {
                        profile_name: profile_name.to_string(),
                        payload,
                    });
                }
            }
        };
    runtime_set_upstream_websocket_io_timeout(
        &mut upstream_socket,
        Some(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms(),
        )),
    )
    .context("failed to configure runtime websocket pre-commit timeout")?;

    if let Err(err) = upstream_socket.send(WsMessage::Text(request_text.to_string().into())) {
        let _ = upstream_socket.close(None);
        websocket_session.reset();
        let transport_error =
            anyhow::anyhow!("failed to send runtime websocket request upstream: {err}");
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            "websocket_upstream_send",
            &transport_error,
        );
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket upstream_send_error profile={profile_name} error={err}"
            ),
        );
        if reuse_existing_session {
            return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name: profile_name.to_string(),
                event: "upstream_send_error",
            });
        }
        return Err(transport_error);
    }

    let mut committed = false;
    let mut first_upstream_frame_seen = false;
    let mut buffered_precommit_text_frames = Vec::new();
    let mut previous_response_owner_recorded = false;
    let mut precommit_hold_count = 0usize;
    loop {
        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(if reuse_existing_session {
                            runtime_proxy_websocket_precommit_progress_timeout_ms()
                        } else {
                            runtime_proxy_stream_idle_timeout_ms()
                        })),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }

                let mut inspected = inspect_runtime_websocket_text_frame(text.as_str());
                if realtime_websocket
                    && inspected
                        .event_type
                        .as_deref()
                        .is_some_and(runtime_realtime_websocket_terminal_event_kind)
                {
                    inspected.terminal_event = true;
                }
                if let Some(turn_state) = inspected.turn_state.as_deref() {
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        Some(turn_state),
                        RuntimeRouteKind::Websocket,
                    )?;
                    upstream_turn_state = Some(turn_state.to_string());
                }

                if !committed {
                    match inspected.retry_kind {
                        Some(RuntimeWebsocketRetryInspectionKind::QuotaBlocked) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                            });
                        }
                        Some(RuntimeWebsocketRetryInspectionKind::Overloaded) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::Overloaded {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                            });
                        }
                        Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::PreviousResponseNotFound {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                                turn_state: upstream_turn_state.clone(),
                            });
                        }
                        None => {}
                    }
                }

                if reuse_existing_session && !committed && inspected.precommit_hold {
                    if precommit_hold_count == 0 {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=websocket precommit_hold profile={profile_name} event_type={}",
                                inspected.event_type.as_deref().unwrap_or("-")
                            ),
                        );
                    }
                    precommit_hold_count = precommit_hold_count.saturating_add(1);
                    buffered_precommit_text_frames.push(RuntimeBufferedWebsocketTextFrame {
                        text,
                        response_ids: inspected.response_ids,
                    });
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    let timeout_ms = runtime_proxy_websocket_precommit_progress_timeout_ms();
                    if elapsed_ms >= u128::from(timeout_ms) {
                        let _ = upstream_socket.close(None);
                        websocket_session.reset();
                        runtime_proxy_log(
                            shared,
                            format!(
                                "websocket_precommit_hold_timeout profile={profile_name} elapsed_ms={elapsed_ms} threshold_ms={timeout_ms} reuse={reuse_existing_session} hold_count={precommit_hold_count}"
                            ),
                        );
                        let transport_error = anyhow::anyhow!(
                            "runtime websocket upstream remained in pre-commit hold beyond the progress deadline after {elapsed_ms} ms"
                        );
                        note_runtime_profile_transport_failure(
                            shared,
                            profile_name,
                            RuntimeRouteKind::Websocket,
                            "websocket_precommit_hold_timeout",
                            &transport_error,
                        );
                        return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                            profile_name: profile_name.to_string(),
                            event: "precommit_hold_timeout",
                        });
                    }
                    continue;
                }

                if !committed {
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id,
                        RuntimeRouteKind::Websocket,
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    let _ = commit_runtime_proxy_profile_selection_with_policy(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        promote_committed_profile,
                    )?;
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket committed profile={profile_name}"
                        ),
                    );
                    committed = true;
                    forward_runtime_proxy_buffered_websocket_text_frames(
                        local_socket,
                        &mut buffered_precommit_text_frames,
                        RuntimeWebsocketResponseBindingContext {
                            shared,
                            profile_name,
                            request_previous_response_id,
                            request_session_id,
                            request_turn_state,
                            response_turn_state: upstream_turn_state.as_deref(),
                        },
                        &mut previous_response_owner_recorded,
                    )?;
                }

                remember_runtime_websocket_response_ids(
                    RuntimeWebsocketResponseBindingContext {
                        shared,
                        profile_name,
                        request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        response_turn_state: upstream_turn_state.as_deref(),
                    },
                    &inspected.response_ids,
                    &mut previous_response_owner_recorded,
                )?;
                local_socket
                    .send(WsMessage::Text(text.into()))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket text frame"
                    })?;
                if inspected.terminal_event {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket terminal_event profile={profile_name} event_type={} precommit_hold_count={precommit_hold_count}",
                            inspected.event_type.as_deref().unwrap_or("-"),
                        ),
                    );
                    websocket_session.store(
                        upstream_socket,
                        profile_name,
                        upstream_turn_state,
                        inflight_guard.take(),
                    );
                    return Ok(RuntimeWebsocketAttempt::Delivered);
                }
            }
            Ok(WsMessage::Binary(payload)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(if reuse_existing_session {
                            runtime_proxy_websocket_precommit_progress_timeout_ms()
                        } else {
                            runtime_proxy_stream_idle_timeout_ms()
                        })),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }
                if !committed {
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id,
                        RuntimeRouteKind::Websocket,
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    let _ = commit_runtime_proxy_profile_selection_with_policy(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        promote_committed_profile,
                    )?;
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket committed_binary profile={profile_name}"
                        ),
                    );
                    committed = true;
                    forward_runtime_proxy_buffered_websocket_text_frames(
                        local_socket,
                        &mut buffered_precommit_text_frames,
                        RuntimeWebsocketResponseBindingContext {
                            shared,
                            profile_name,
                            request_previous_response_id,
                            request_session_id,
                            request_turn_state,
                            response_turn_state: upstream_turn_state.as_deref(),
                        },
                        &mut previous_response_owner_recorded,
                    )?;
                }
                local_socket
                    .send(WsMessage::Binary(payload))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket binary frame"
                    })?;
            }
            Ok(WsMessage::Ping(payload)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(if reuse_existing_session {
                            runtime_proxy_websocket_precommit_progress_timeout_ms()
                        } else {
                            runtime_proxy_stream_idle_timeout_ms()
                        })),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to upstream websocket ping")?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(if reuse_existing_session {
                            runtime_proxy_websocket_precommit_progress_timeout_ms()
                        } else {
                            runtime_proxy_stream_idle_timeout_ms()
                        })),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }
            }
            Ok(WsMessage::Close(frame)) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=upstream_close_before_terminal elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_close_before_completed profile={profile_name}"
                    ),
                );
                let _ = frame;
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_close",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_close_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=connection_closed elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_connection_closed profile={profile_name}"
                    ),
                );
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_connection_closed",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "connection_closed_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(err) => {
                websocket_session.reset();
                if !committed
                    && reuse_existing_session
                    && precommit_hold_count > 0
                    && runtime_websocket_timeout_error(&err)
                {
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    let timeout_ms = runtime_proxy_websocket_precommit_progress_timeout_ms();
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_precommit_hold_timeout profile={profile_name} elapsed_ms={elapsed_ms} threshold_ms={timeout_ms} reuse={reuse_existing_session} hold_count={precommit_hold_count}"
                        ),
                    );
                    let transport_error = anyhow::anyhow!(
                        "runtime websocket upstream remained in pre-commit hold beyond the progress deadline after {elapsed_ms} ms: {err}"
                    );
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        "websocket_precommit_hold_timeout",
                        &transport_error,
                    );
                    if let Some(started_at) = reuse_started_at {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "websocket_reuse_watchdog profile={profile_name} event=precommit_hold_timeout elapsed_ms={} committed={committed}",
                                started_at.elapsed().as_millis()
                            ),
                        );
                    }
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "precommit_hold_timeout",
                    });
                }
                if !committed && !first_upstream_frame_seen && runtime_websocket_timeout_error(&err)
                {
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_precommit_frame_timeout profile={profile_name} event=no_first_upstream_frame_before_deadline elapsed_ms={elapsed_ms} reuse={reuse_existing_session}"
                        ),
                    );
                    let transport_error = anyhow::anyhow!(
                        "runtime websocket upstream produced no first frame before the pre-commit deadline: {err}"
                    );
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        "websocket_first_frame_timeout",
                        &transport_error,
                    );
                    if reuse_existing_session {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "websocket_reuse_watchdog profile={profile_name} event=no_first_upstream_frame_before_deadline elapsed_ms={elapsed_ms} committed={committed}"
                            ),
                        );
                        return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                            profile_name: profile_name.to_string(),
                            event: "no_first_upstream_frame_before_deadline",
                        });
                    }
                    return Err(transport_error);
                }
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=read_error elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_read_error profile={profile_name} error={err}"
                    ),
                );
                let transport_error = anyhow::anyhow!(
                    "runtime websocket upstream failed before response.completed: {err}"
                );
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_read",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_read_error",
                    });
                }
                return Err(transport_error);
            }
        }
    }
}

pub(super) fn proxy_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    let mut request = request.clone();
    let request_requires_previous_response_affinity =
        runtime_request_requires_previous_response_affinity(&request);
    let mut previous_response_id = runtime_request_previous_response_id(&request);
    let mut request_turn_state = runtime_request_turn_state(&request);
    let explicit_request_session_id = runtime_request_explicit_session_id(&request);
    let request_session_id = runtime_request_session_id(&request);
    let mut bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Responses)
        })
        .transpose()?
        .flatten();
    let mut trusted_previous_response_affinity = runtime_previous_response_affinity_is_trusted(
        shared,
        previous_response_id.as_deref(),
        bound_profile.as_deref(),
    )?;
    if request_turn_state.is_none()
        && let Some(turn_state) = runtime_previous_response_turn_state(
            shared,
            previous_response_id.as_deref(),
            bound_profile.as_deref(),
        )?
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http route=responses previous_response_turn_state_rehydrated response_id={} profile={} turn_state={turn_state}",
                previous_response_id.as_deref().unwrap_or("-"),
                bound_profile.as_deref().unwrap_or("-"),
            ),
        );
        request_turn_state = Some(turn_state);
    }
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut bound_session_profile = None;
    let mut compact_followup_profile = None;
    let mut compact_session_profile = None;
    let mut session_profile = None;
    let mut pinned_profile = None;
    macro_rules! recompute_route_affinity {
        ($reason:expr) => {
            refresh_and_log_runtime_response_route_affinity(
                shared,
                request_id,
                None,
                $reason,
                previous_response_id.as_deref(),
                bound_profile.as_deref(),
                turn_state_profile.as_deref(),
                request_turn_state.as_deref(),
                request_session_id.as_deref(),
                explicit_request_session_id.as_ref(),
                None,
                &mut bound_session_profile,
                &mut compact_followup_profile,
                &mut compact_session_profile,
                &mut session_profile,
                &mut pinned_profile,
            )
        };
    }
    recompute_route_affinity!("initial")?;
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure: Option<(RuntimeUpstreamFailureResponse, bool)> = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;
    let mut saw_inflight_saturation = false;
    let mut selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    let mut previous_response_fresh_fallback_used = false;
    let mut saw_previous_response_not_found = false;
    macro_rules! runtime_candidate_affinity {
        ($route_kind:expr, $candidate_name:expr, $strict_affinity_profile:expr $(,)?) => {
            RuntimeCandidateAffinity {
                route_kind: $route_kind,
                candidate_name: $candidate_name,
                strict_affinity_profile: $strict_affinity_profile,
                pinned_profile: pinned_profile.as_deref(),
                turn_state_profile: turn_state_profile.as_deref(),
                session_profile: session_profile.as_deref(),
                trusted_previous_response_affinity,
            }
        };
    }
    macro_rules! runtime_previous_response_fresh_fallback_state {
        () => {
            RuntimePreviousResponseFreshFallbackState {
                previous_response_id: &mut previous_response_id,
                request_turn_state: &mut request_turn_state,
                previous_response_fresh_fallback_used: &mut previous_response_fresh_fallback_used,
                saw_previous_response_not_found: &mut saw_previous_response_not_found,
                previous_response_retry_candidate: &mut previous_response_retry_candidate,
                previous_response_retry_index: &mut previous_response_retry_index,
                candidate_turn_state_retry_profile: &mut candidate_turn_state_retry_profile,
                candidate_turn_state_retry_value: &mut candidate_turn_state_retry_value,
                trusted_previous_response_affinity: &mut trusted_previous_response_affinity,
                bound_profile: &mut bound_profile,
                pinned_profile: &mut pinned_profile,
                turn_state_profile: &mut turn_state_profile,
                session_profile: &mut session_profile,
                excluded_profiles: &mut excluded_profiles,
                last_failure: &mut last_failure,
                selection_started_at: &mut selection_started_at,
                selection_attempts: &mut selection_attempts,
            }
        };
    }

    loop {
        let pressure_mode =
            runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Responses);
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            runtime_proxy_has_continuation_priority(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
            ),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_request_requires_previous_response_affinity(&request)
                && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http previous_response_fresh_fallback reason=precommit_budget_exhausted"
                    ),
                );
                apply_runtime_responses_previous_response_fresh_fallback(
                    &mut request,
                    fresh_request,
                    runtime_previous_response_fresh_fallback_state!(),
                );
                recompute_route_affinity!("previous_response_fresh_fallback")?;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=precommit_budget_exhausted"
                    ),
                );
                return Ok(runtime_proxy_final_responses_failure_reply(
                    last_failure,
                    saw_inflight_saturation,
                ));
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Responses,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http direct_current_profile_fallback profile={current_profile} reason=precommit_budget_exhausted"
                    ),
                );
                match attempt_runtime_responses_request(
                    request_id,
                    &request,
                    shared,
                    &current_profile,
                    request_turn_state.as_deref(),
                )? {
                    RuntimeResponsesAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        if saw_previous_response_not_found {
                            remember_runtime_successful_previous_response_owner(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                RuntimeRouteKind::Responses,
                            )?;
                        }
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Responses,
                        )?;
                        let _ = release_runtime_compact_lineage(
                            shared,
                            &profile_name,
                            request_session_id.as_deref(),
                            request_turn_state.as_deref(),
                            "response_committed_post_commit",
                        );
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http committed profile={profile_name} via=direct_current_profile_fallback"
                            ),
                        );
                        return Ok(response);
                    }
                    RuntimeResponsesAttempt::QuotaBlocked {
                        profile_name,
                        response,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Responses,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(response);
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request) =
                            runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_responses_previous_response_fresh_fallback(
                                &mut request,
                                fresh_request,
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        if !runtime_has_route_eligible_quota_fallback(
                            shared,
                            &profile_name,
                            &BTreeSet::new(),
                            RuntimeRouteKind::Responses,
                        )? {
                            return Ok(response);
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
                        continue;
                    }
                    RuntimeResponsesAttempt::PreviousResponseNotFound {
                        profile_name,
                        response,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure =
                                Some((RuntimeUpstreamFailureResponse::Http(response), false));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry && !request_requires_previous_response_affinity {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Responses,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            false,
                            &mut turn_state_profile,
                            Some(&mut compact_followup_profile),
                        );
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Http(response), false));
                        continue;
                    }
                    RuntimeResponsesAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Responses,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(RuntimeResponsesReply::Buffered(
                                build_runtime_proxy_json_error_parts(
                                    503,
                                    "service_unavailable",
                                    runtime_proxy_local_selection_failure_message(),
                                ),
                            ));
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request) =
                            runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_responses_previous_response_fresh_fallback(
                                &mut request,
                                fresh_request,
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            return Ok(runtime_proxy_final_responses_failure_reply(
                last_failure,
                saw_inflight_saturation,
            ));
        }

        let Some(candidate_name) = select_runtime_response_candidate_for_route_with_selection(
            shared,
            RuntimeResponseCandidateSelection {
                excluded_profiles: &excluded_profiles,
                strict_affinity_profile: compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                pinned_profile: pinned_profile.as_deref(),
                turn_state_profile: turn_state_profile.as_deref(),
                session_profile: session_profile.as_deref(),
                discover_previous_response_owner: previous_response_id.is_some(),
                previous_response_id: previous_response_id.as_deref(),
                route_kind: RuntimeRouteKind::Responses,
            },
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http candidate_exhausted last_failure={}",
                    match &last_failure {
                        Some((RuntimeUpstreamFailureResponse::Http(_), _)) => "http",
                        Some((RuntimeUpstreamFailureResponse::Websocket(_), _)) => "websocket",
                        None => "none",
                    }
                ),
            );
            if runtime_proxy_maybe_wait_for_interactive_inflight_relief(
                RuntimeInflightReliefWait {
                    request_id,
                    request: &request,
                    shared,
                    excluded_profiles: &excluded_profiles,
                    route_kind: RuntimeRouteKind::Responses,
                    selection_started_at,
                    continuation: runtime_proxy_has_continuation_priority(
                        previous_response_id.as_deref(),
                        pinned_profile.as_deref(),
                        request_turn_state.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                    ),
                    wait_affinity_owner: runtime_wait_affinity_owner(
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        pinned_profile.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                        trusted_previous_response_affinity,
                    ),
                },
            )? {
                continue;
            }
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_request_requires_previous_response_affinity(&request)
                && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http previous_response_fresh_fallback reason=candidate_exhausted"
                    ),
                );
                apply_runtime_responses_previous_response_fresh_fallback(
                    &mut request,
                    fresh_request,
                    runtime_previous_response_fresh_fallback_state!(),
                );
                recompute_route_affinity!("previous_response_fresh_fallback")?;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=candidate_exhausted"
                    ),
                );
                return Ok(runtime_proxy_final_responses_failure_reply(
                    last_failure,
                    saw_inflight_saturation,
                ));
            }
            let remaining_cold_start_profiles =
                runtime_remaining_sync_probe_cold_start_profiles_for_route(
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Responses,
                )?;
            if remaining_cold_start_profiles > 0 {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http candidate_exhausted_continue route=responses remaining_cold_start_profiles={remaining_cold_start_profiles}"
                    ),
                );
                runtime_proxy_sync_probe_pressure_pause(shared, RuntimeRouteKind::Responses);
                continue;
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Responses,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http direct_current_profile_fallback profile={current_profile} reason=candidate_exhausted"
                    ),
                );
                match attempt_runtime_responses_request(
                    request_id,
                    &request,
                    shared,
                    &current_profile,
                    request_turn_state.as_deref(),
                )? {
                    RuntimeResponsesAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        if saw_previous_response_not_found {
                            remember_runtime_successful_previous_response_owner(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                RuntimeRouteKind::Responses,
                            )?;
                        }
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Responses,
                        )?;
                        let _ = release_runtime_compact_lineage(
                            shared,
                            &profile_name,
                            request_session_id.as_deref(),
                            request_turn_state.as_deref(),
                            "response_committed_post_commit",
                        );
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http committed profile={profile_name} via=direct_current_profile_fallback"
                            ),
                        );
                        return Ok(response);
                    }
                    RuntimeResponsesAttempt::QuotaBlocked {
                        profile_name,
                        response,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Responses,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(response);
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request) =
                            runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_responses_previous_response_fresh_fallback(
                                &mut request,
                                fresh_request,
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        if !runtime_has_route_eligible_quota_fallback(
                            shared,
                            &profile_name,
                            &BTreeSet::new(),
                            RuntimeRouteKind::Responses,
                        )? {
                            return Ok(response);
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
                        continue;
                    }
                    RuntimeResponsesAttempt::PreviousResponseNotFound {
                        profile_name,
                        response,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure =
                                Some((RuntimeUpstreamFailureResponse::Http(response), false));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry && !request_requires_previous_response_affinity {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Responses,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            false,
                            &mut turn_state_profile,
                            Some(&mut compact_followup_profile),
                        );
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Http(response), false));
                        continue;
                    }
                    RuntimeResponsesAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Responses,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(RuntimeResponsesReply::Buffered(
                                build_runtime_proxy_json_error_parts(
                                    503,
                                    "service_unavailable",
                                    runtime_proxy_local_selection_failure_message(),
                                ),
                            ));
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request) =
                            runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_responses_previous_response_fresh_fallback(
                                &mut request,
                                fresh_request,
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            return Ok(runtime_proxy_final_responses_failure_reply(
                last_failure,
                saw_inflight_saturation,
            ));
        };
        selection_attempts = selection_attempts.saturating_add(1);
        let turn_state_override =
            if candidate_turn_state_retry_profile.as_deref() == Some(candidate_name.as_str()) {
                candidate_turn_state_retry_value.as_deref()
            } else {
                request_turn_state.as_deref()
            };
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                candidate_name,
                pinned_profile,
                turn_state_profile,
                turn_state_override,
                excluded_profiles.len()
            ),
        );
        if previous_response_id.is_none()
            && pinned_profile.is_none()
            && turn_state_profile.is_none()
            && runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate_name,
                "responses_http",
            )?
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http profile_inflight_saturated profile={candidate_name} hard_limit={}",
                    runtime_proxy_profile_inflight_hard_limit(),
                ),
            );
            saw_inflight_saturation = true;
            if runtime_proxy_maybe_wait_for_interactive_inflight_relief(
                RuntimeInflightReliefWait {
                    request_id,
                    request: &request,
                    shared,
                    excluded_profiles: &excluded_profiles,
                    route_kind: RuntimeRouteKind::Responses,
                    selection_started_at,
                    continuation: runtime_proxy_has_continuation_priority(
                        previous_response_id.as_deref(),
                        pinned_profile.as_deref(),
                        request_turn_state.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                    ),
                    wait_affinity_owner: runtime_wait_affinity_owner(
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        pinned_profile.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                        trusted_previous_response_affinity,
                    ),
                },
            )? {
                continue;
            }
            excluded_profiles.insert(candidate_name);
            continue;
        }

        match attempt_runtime_responses_request(
            request_id,
            &request,
            shared,
            &candidate_name,
            turn_state_override,
        )? {
            RuntimeResponsesAttempt::Success {
                profile_name,
                response,
            } => {
                if saw_previous_response_not_found {
                    remember_runtime_successful_previous_response_owner(
                        shared,
                        &profile_name,
                        previous_response_id.as_deref(),
                        RuntimeRouteKind::Responses,
                    )?;
                }
                commit_runtime_proxy_profile_selection_with_notice(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Responses,
                )?;
                let _ = release_runtime_compact_lineage(
                    shared,
                    &profile_name,
                    request_session_id.as_deref(),
                    request_turn_state.as_deref(),
                    "response_committed_post_commit",
                );
                runtime_proxy_log(
                    shared,
                    format!("request={request_id} transport=http committed profile={profile_name}"),
                );
                return Ok(response);
            }
            RuntimeResponsesAttempt::QuotaBlocked {
                profile_name,
                response,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http quota_blocked profile={profile_name}"
                    ),
                );
                let quota_message =
                    extract_runtime_proxy_quota_message_from_response_reply(&response);
                mark_runtime_profile_quota_quarantine(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Responses,
                    quota_message.as_deref(),
                )?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    runtime_candidate_affinity!(
                        RuntimeRouteKind::Responses,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                    ),
                    request_requires_previous_response_affinity,
                ) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http upstream_usage_limit_passthrough route=responses profile={profile_name} reason=hard_affinity"
                        ),
                    );
                    return Ok(response);
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name}"
                        ),
                    );
                }
                if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                    previous_response_id.as_deref(),
                    trusted_previous_response_affinity,
                    previous_response_fresh_fallback_used,
                ) && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_fresh_fallback reason=quota_blocked"
                        ),
                    );
                    apply_runtime_responses_previous_response_fresh_fallback(
                        &mut request,
                        fresh_request,
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                if !runtime_has_route_eligible_quota_fallback(
                    shared,
                    &profile_name,
                    &BTreeSet::new(),
                    RuntimeRouteKind::Responses,
                )? {
                    return Ok(response);
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
            }
            RuntimeResponsesAttempt::LocalSelectionBlocked {
                profile_name,
                reason,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http local_selection_blocked profile={profile_name} route=responses reason={reason}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    runtime_candidate_affinity!(
                        RuntimeRouteKind::Responses,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                    ),
                    request_requires_previous_response_affinity,
                ) {
                    return Ok(RuntimeResponsesReply::Buffered(
                        build_runtime_proxy_json_error_parts(
                            503,
                            "service_unavailable",
                            runtime_proxy_local_selection_failure_message(),
                        ),
                    ));
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason}"
                        ),
                    );
                }
                if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                    previous_response_id.as_deref(),
                    trusted_previous_response_affinity,
                    previous_response_fresh_fallback_used,
                ) && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_fresh_fallback reason={reason}"
                        ),
                    );
                    apply_runtime_responses_previous_response_fresh_fallback(
                        &mut request,
                        fresh_request,
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name,
                response,
                turn_state,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?}",
                        turn_state
                    ),
                );
                saw_previous_response_not_found = true;
                if previous_response_retry_candidate.as_deref() != Some(profile_name.as_str()) {
                    previous_response_retry_candidate = Some(profile_name.clone());
                    previous_response_retry_index = 0;
                }
                let has_turn_state_retry = turn_state.is_some();
                if has_turn_state_retry {
                    candidate_turn_state_retry_profile = Some(profile_name.clone());
                    candidate_turn_state_retry_value = turn_state;
                }
                if has_turn_state_retry
                    && let Some(delay) =
                        runtime_previous_response_retry_delay(previous_response_retry_index)
                {
                    previous_response_retry_index += 1;
                    last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), false));
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry",
                            delay.as_millis()
                        ),
                    );
                    continue;
                }
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                if !has_turn_state_retry && !request_requires_previous_response_affinity {
                    let _ = clear_runtime_stale_previous_response_binding(
                        shared,
                        &profile_name,
                        previous_response_id.as_deref(),
                    )?;
                }
                let released_affinity = release_runtime_previous_response_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                    RuntimeRouteKind::Responses,
                )?;
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_affinity_released profile={profile_name}"
                        ),
                    );
                }
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    false,
                    &mut turn_state_profile,
                    Some(&mut compact_followup_profile),
                );
                trusted_previous_response_affinity = false;
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), false));
            }
        }
    }
}

pub(super) fn attempt_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<RuntimeResponsesAttempt> {
    let request_session_id = runtime_request_session_id(request);
    let request_previous_response_id = runtime_request_previous_response_id(request);
    let request_turn_state = runtime_request_turn_state(request);
    let (initial_quota_summary, initial_quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Responses)?;
    if (request_previous_response_id.is_some()
        || request_session_id.is_some()
        || request_turn_state.is_some())
        && matches!(
            initial_quota_source,
            Some(RuntimeQuotaSource::PersistedSnapshot)
        )
        && let Some(reason) =
            runtime_quota_precommit_guard_reason(initial_quota_summary, RuntimeRouteKind::Responses)
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason={reason} quota_source={} {}",
                initial_quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(initial_quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }
    let has_alternative_quota_profile = runtime_has_route_eligible_quota_fallback(
        shared,
        profile_name,
        &BTreeSet::new(),
        RuntimeRouteKind::Responses,
    )?;
    let (quota_summary, quota_source) = ensure_runtime_profile_precommit_quota_ready(
        shared,
        profile_name,
        RuntimeRouteKind::Responses,
        "responses_precommit_reprobe",
    )?;
    if runtime_quota_summary_requires_live_source_after_probe(
        quota_summary,
        quota_source,
        RuntimeRouteKind::Responses,
    ) && has_alternative_quota_profile
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason=quota_windows_unavailable_after_reprobe quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: "quota_windows_unavailable_after_reprobe",
        });
    }
    if let Some(reason) =
        runtime_quota_precommit_guard_reason(quota_summary, RuntimeRouteKind::Responses)
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason={reason} quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }
    let inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "responses_http")?;
    let mut inflight_guard = Some(inflight_guard);
    let mut recovery_steps = [
        RuntimeProfileUnauthorizedRecoveryStep::Reload,
        RuntimeProfileUnauthorizedRecoveryStep::Refresh,
    ]
    .into_iter();
    loop {
        let response = send_runtime_proxy_upstream_responses_request(
            request_id,
            request,
            shared,
            profile_name,
            turn_state_override,
        )
        .inspect_err(|err| {
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Responses,
                "responses_upstream_request",
                err,
            );
        })?;
        let response_turn_state =
            runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
                .inspect_err(|err| {
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Responses,
                        "responses_buffer_response",
                        err,
                    );
                })?;
            if status == 401
                && runtime_try_recover_profile_auth_from_unauthorized_steps(
                    request_id,
                    shared,
                    profile_name,
                    RuntimeRouteKind::Responses,
                    &mut recovery_steps,
                )
            {
                continue;
            }
            let retryable_quota = matches!(status, 403 | 429)
                && extract_runtime_proxy_quota_message(&parts.body).is_some();
            let retryable_previous = status == 400
                && extract_runtime_proxy_previous_response_message(&parts.body).is_some();
            let response = RuntimeResponsesReply::Buffered(parts);

            if retryable_quota {
                return Ok(RuntimeResponsesAttempt::QuotaBlocked {
                    profile_name: profile_name.to_string(),
                    response,
                });
            }
            if retryable_previous {
                return Ok(RuntimeResponsesAttempt::PreviousResponseNotFound {
                    profile_name: profile_name.to_string(),
                    response,
                    turn_state: response_turn_state,
                });
            }
            if matches!(status, 401 | 403) {
                note_runtime_profile_auth_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Responses,
                    status,
                );
            }

            return Ok(RuntimeResponsesAttempt::Success {
                profile_name: profile_name.to_string(),
                response,
            });
        }
        return prepare_runtime_proxy_responses_success(
            RuntimeResponsesSuccessContext {
                request_id,
                request_previous_response_id: runtime_request_previous_response_id(request)
                    .as_deref(),
                request_session_id: request_session_id.as_deref(),
                request_turn_state: runtime_request_turn_state(request).as_deref(),
                turn_state_override,
                shared,
                profile_name,
                inflight_guard: inflight_guard
                    .take()
                    .expect("responses inflight guard should be present"),
            },
            response,
        )
        .inspect_err(|err| {
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Responses,
                "responses_prepare_success",
                err,
            );
        });
    }
}

pub(super) fn next_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let sync_probe_pressure_mode =
        runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let allow_disk_auth_fallback = !sync_probe_pressure_mode;
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let (
        state,
        current_profile,
        include_code_review,
        upstream_base_url,
        cached_reports,
        mut cached_usage_snapshots,
        profile_usage_auth,
        mut retry_backoff_until,
        mut transport_backoff_until,
        mut route_circuit_open_until,
        profile_inflight,
        profile_health,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.include_code_review,
            runtime.upstream_base_url.clone(),
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
        )
    };

    let mut reports = Vec::new();
    let mut cold_start_probe_jobs = Vec::new();
    for (order_index, name) in active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .enumerate()
    {
        if excluded_profiles.contains(&name) {
            continue;
        }
        let Some(profile) = state.profiles.get(&name) else {
            continue;
        };
        if let Some(entry) = cached_reports.get(&name) {
            reports.push(RunProfileProbeReport {
                name: name.clone(),
                order_index,
                auth: entry.auth.clone(),
                result: entry.result.clone(),
            });
            if runtime_profile_probe_cache_freshness(entry, now)
                != RuntimeProbeCacheFreshness::Fresh
            {
                if profile.provider.supports_codex_runtime() {
                    schedule_runtime_probe_refresh(shared, &name, &profile.codex_home);
                }
            }
        } else {
            let auth = runtime_profile_auth_summary_for_selection_with_policy(
                &name,
                &profile.codex_home,
                &profile_usage_auth,
                &cached_reports,
                allow_disk_auth_fallback,
            )
            .unwrap_or_else(runtime_profile_uncached_auth_summary_for_selection);
            reports.push(RunProfileProbeReport {
                name: name.clone(),
                order_index,
                auth,
                result: Err("runtime quota snapshot unavailable".to_string()),
            });
            cold_start_probe_jobs.push(RunProfileProbeJob {
                name,
                order_index,
                provider: profile.provider.clone(),
                codex_home: profile.codex_home.clone(),
            });
        }
    }

    cold_start_probe_jobs.sort_by_key(|job| {
        let quota_summary = cached_usage_snapshots
            .get(&job.name)
            .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
            .map(|snapshot| runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now))
            .unwrap_or(RuntimeQuotaSummary {
                five_hour: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                weekly: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                route_band: RuntimeQuotaPressureBand::Unknown,
            });
        (
            runtime_quota_pressure_sort_key_for_route_from_summary(quota_summary),
            job.order_index,
        )
    });
    let request_probe_jobs = cold_start_probe_jobs
        .iter()
        .filter(|job| {
            !cached_usage_snapshots
                .get(&job.name)
                .is_some_and(|snapshot| {
                    runtime_snapshot_blocks_same_request_cold_start_probe(snapshot, route_kind, now)
                })
        })
        .cloned()
        .collect::<Vec<_>>();

    reports.sort_by_key(|report| report.order_index);
    let mut candidates = ready_profile_candidates(
        &reports,
        include_code_review,
        Some(current_profile.as_str()),
        &state,
        Some(&cached_usage_snapshots),
    );

    let best_candidate_order_index = candidates
        .iter()
        .filter(|candidate| !excluded_profiles.contains(&candidate.name))
        .filter(|candidate| {
            !runtime_profile_name_in_selection_backoff(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                &route_circuit_open_until,
                route_kind,
                now,
            )
        })
        .filter(|candidate| {
            !runtime_profile_auth_failure_active_with_auth_cache(
                &profile_health,
                &profile_usage_auth,
                &candidate.name,
                now,
            )
        })
        .filter(|candidate| {
            runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight)
                < inflight_soft_limit
        })
        .map(|candidate| candidate.order_index)
        .min();
    let should_sync_probe_cold_start = !sync_probe_pressure_mode
        && !request_probe_jobs.is_empty()
        && (candidates.is_empty()
            || best_candidate_order_index.is_none()
            || best_candidate_order_index.is_some_and(|best_order_index| {
                request_probe_jobs
                    .iter()
                    .any(|job| job.order_index < best_order_index)
            }));
    if sync_probe_pressure_mode && !request_probe_jobs.is_empty() {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_sync_probe route={} reason=pressure_mode cold_start_jobs={}",
                runtime_route_kind_label(route_kind),
                request_probe_jobs.len(),
            ),
        );
    }

    if should_sync_probe_cold_start {
        let base_url = Some(upstream_base_url.clone());
        let sync_jobs = request_probe_jobs
            .iter()
            .filter(|job| {
                candidates.is_empty()
                    || best_candidate_order_index.is_none()
                    || best_candidate_order_index
                        .is_some_and(|best_order_index| job.order_index < best_order_index)
            })
            .take(RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT)
            .map(|job| RunProfileProbeJob {
                name: job.name.clone(),
                order_index: job.order_index,
                provider: job.provider.clone(),
                codex_home: job.codex_home.clone(),
            })
            .collect::<Vec<_>>();
        let probed_names = sync_jobs
            .iter()
            .map(|job| job.name.clone())
            .collect::<BTreeSet<_>>();
        let fresh_reports = map_parallel(sync_jobs, |job| {
            let auth = job.provider.auth_summary(&job.codex_home);
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
        });

        for report in &fresh_reports {
            apply_runtime_profile_probe_result(
                shared,
                &report.name,
                report.auth.clone(),
                report.result.clone(),
            )?;
        }
        {
            let runtime = shared
                .runtime
                .lock()
                .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
            cached_usage_snapshots = runtime.profile_usage_snapshots.clone();
            retry_backoff_until = runtime.profile_retry_backoff_until.clone();
            transport_backoff_until = runtime.profile_transport_backoff_until.clone();
            route_circuit_open_until = runtime.profile_route_circuit_open_until.clone();
        }

        for fresh_report in fresh_reports {
            if let Some(existing) = reports
                .iter_mut()
                .find(|report| report.name == fresh_report.name)
            {
                *existing = fresh_report;
            }
        }
        reports.sort_by_key(|report| report.order_index);
        candidates = ready_profile_candidates(
            &reports,
            include_code_review,
            Some(current_profile.as_str()),
            &state,
            Some(&cached_usage_snapshots),
        );
        for job in cold_start_probe_jobs
            .into_iter()
            .filter(|job| !probed_names.contains(&job.name))
        {
            schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
        }
    } else {
        if sync_probe_pressure_mode && !cold_start_probe_jobs.is_empty() {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_sync_probe route={} reason=pressure_mode cold_start_profiles={}",
                    runtime_route_kind_label(route_kind),
                    cold_start_probe_jobs.len()
                ),
            );
        }
        for job in cold_start_probe_jobs {
            schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
        }
    }
    let available_candidates = candidates
        .into_iter()
        .enumerate()
        .filter(|(_, candidate)| !excluded_profiles.contains(&candidate.name))
        .collect::<Vec<_>>();

    let mut ready_candidates = available_candidates
        .iter()
        .filter(|(_, candidate)| {
            !runtime_profile_name_in_selection_backoff(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                &route_circuit_open_until,
                route_kind,
                now,
            )
        })
        .collect::<Vec<_>>();
    ready_candidates.sort_by_key(|(index, candidate)| {
        (
            candidate.provider_priority,
            runtime_quota_pressure_sort_key_for_route(&candidate.usage, route_kind),
            runtime_quota_source_sort_key(route_kind, candidate.quota_source),
            runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
            runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
            *index,
            runtime_profile_selection_jitter(shared, &candidate.name, route_kind),
        )
    });
    for (index, candidate) in ready_candidates {
        let inflight = runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight);
        let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=auth_failure_backoff inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    inflight,
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(runtime_quota_summary_for_route(
                        &candidate.usage,
                        route_kind
                    )),
                ),
            );
            continue;
        }
        if matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && let Some(reason) = runtime_quota_precommit_guard_reason(quota_summary, route_kind)
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason={} inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    reason,
                    inflight,
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        if inflight >= inflight_soft_limit {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=profile_inflight_soft_limit inflight={} soft_limit={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    inflight,
                    inflight_soft_limit,
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        if !reserve_runtime_profile_route_circuit_half_open_probe(
            shared,
            &candidate.name,
            route_kind,
        )? {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    inflight,
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile={} mode=ready inflight={} health={} order={} {}",
                runtime_route_kind_label(route_kind),
                candidate.name,
                inflight,
                runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
                index,
                format_args!(
                    "quota_source={} {}",
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary)
                ),
            ),
        );
        return Ok(Some(candidate.name.clone()));
    }

    let mut fallback_candidates = available_candidates.into_iter().collect::<Vec<_>>();
    fallback_candidates.sort_by_key(|(index, candidate)| {
        (
            runtime_profile_backoff_sort_key(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                &route_circuit_open_until,
                route_kind,
                now,
            ),
            candidate.provider_priority,
            runtime_quota_pressure_sort_key_for_route(&candidate.usage, route_kind),
            runtime_quota_source_sort_key(route_kind, candidate.quota_source),
            runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
            runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
            *index,
            runtime_profile_selection_jitter(shared, &candidate.name, route_kind),
        )
    });
    let mut fallback = None;
    for (index, candidate) in fallback_candidates {
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=auth_failure_backoff inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(runtime_quota_summary_for_route(
                        &candidate.usage,
                        route_kind
                    )),
                ),
            );
            continue;
        }
        let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
        if matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && let Some(reason) = runtime_quota_precommit_guard_reason(quota_summary, route_kind)
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason={} inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    reason,
                    runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        if !reserve_runtime_profile_route_circuit_half_open_probe(
            shared,
            &candidate.name,
            route_kind,
        )? {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile={} mode=backoff inflight={} health={} backoff={:?} order={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                candidate.name,
                runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
                runtime_profile_backoff_sort_key(
                    &candidate.name,
                    &retry_backoff_until,
                    &transport_backoff_until,
                    &route_circuit_open_until,
                    route_kind,
                    now,
                ),
                index,
                runtime_quota_source_label(candidate.quota_source),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        fallback = Some(candidate.name);
        break;
    }

    if fallback.is_none() {
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile=none mode=exhausted excluded_count={}",
                runtime_route_kind_label(route_kind),
                excluded_profiles.len()
            ),
        );
    }

    Ok(fallback)
}

pub(super) fn runtime_remaining_sync_probe_cold_start_profiles_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<usize> {
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let (state, current_profile, cached_reports, cached_usage_snapshots, profile_usage_auth) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_usage_auth.clone(),
        )
    };
    let now = Local::now().timestamp();

    Ok(active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .filter(|name| !excluded_profiles.contains(name))
        .filter(|name| {
            state.profiles.get(name).is_some_and(|profile| {
                runtime_profile_auth_summary_for_selection_with_policy(
                    name,
                    &profile.codex_home,
                    &profile_usage_auth,
                    &cached_reports,
                    allow_disk_auth_fallback,
                )
                .is_some_and(|summary| summary.quota_compatible)
            })
        })
        .filter(|name| !cached_reports.contains_key(name))
        .filter(|name| {
            !cached_usage_snapshots.get(name).is_some_and(|snapshot| {
                runtime_snapshot_blocks_same_request_cold_start_probe(snapshot, route_kind, now)
            })
        })
        .count())
}

pub(super) fn runtime_waitable_inflight_candidates_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    wait_affinity_owner: Option<&str>,
) -> Result<BTreeSet<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let (
        state,
        current_profile,
        include_code_review,
        cached_reports,
        cached_usage_snapshots,
        profile_usage_auth,
        retry_backoff_until,
        transport_backoff_until,
        route_circuit_open_until,
        profile_inflight,
        profile_health,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.include_code_review,
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
        )
    };

    let mut waitable_profiles = BTreeSet::new();
    let mut reports = Vec::new();
    for (order_index, name) in active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .enumerate()
    {
        if excluded_profiles.contains(&name) {
            continue;
        }
        if wait_affinity_owner.is_some_and(|owner| owner != name) {
            continue;
        }
        let Some(entry) = cached_reports.get(&name) else {
            continue;
        };
        reports.push(RunProfileProbeReport {
            name,
            order_index,
            auth: entry.auth.clone(),
            result: entry.result.clone(),
        });
    }

    for candidate in ready_profile_candidates(
        &reports,
        include_code_review,
        Some(current_profile.as_str()),
        &state,
        Some(&cached_usage_snapshots),
    ) {
        if excluded_profiles.contains(&candidate.name) {
            continue;
        }
        if runtime_profile_name_in_selection_backoff(
            &candidate.name,
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            route_kind,
            now,
        ) {
            continue;
        }
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
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
        if runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight)
            >= inflight_soft_limit
        {
            waitable_profiles.insert(candidate.name.clone());
        }
    }

    Ok(waitable_profiles)
}

pub(super) fn runtime_any_waited_candidate_relieved(
    shared: &RuntimeRotationProxyShared,
    waited_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    if waited_profiles.is_empty() {
        return Ok(false);
    }
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let (
        state,
        current_profile,
        include_code_review,
        cached_reports,
        cached_usage_snapshots,
        profile_usage_auth,
        retry_backoff_until,
        transport_backoff_until,
        route_circuit_open_until,
        profile_inflight,
        profile_health,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.include_code_review,
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
        )
    };

    let mut reports = Vec::new();
    for (order_index, name) in active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .enumerate()
    {
        if !waited_profiles.contains(&name) {
            continue;
        }
        let Some(entry) = cached_reports.get(&name) else {
            continue;
        };
        reports.push(RunProfileProbeReport {
            name,
            order_index,
            auth: entry.auth.clone(),
            result: entry.result.clone(),
        });
    }

    for candidate in ready_profile_candidates(
        &reports,
        include_code_review,
        Some(current_profile.as_str()),
        &state,
        Some(&cached_usage_snapshots),
    ) {
        if !waited_profiles.contains(&candidate.name) {
            continue;
        }
        if runtime_profile_name_in_selection_backoff(
            &candidate.name,
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            route_kind,
            now,
        ) {
            continue;
        }
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
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
        if runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight)
            < inflight_soft_limit
        {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(super) struct RuntimeInflightReliefWait<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeRotationProxyShared,
    excluded_profiles: &'a BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    selection_started_at: Instant,
    continuation: bool,
    wait_affinity_owner: Option<&'a str>,
}

impl<'a> RuntimeInflightReliefWait<'a> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn new(
        request_id: u64,
        request: &'a RuntimeProxyRequest,
        shared: &'a RuntimeRotationProxyShared,
        excluded_profiles: &'a BTreeSet<String>,
        route_kind: RuntimeRouteKind,
        selection_started_at: Instant,
        continuation: bool,
        wait_affinity_owner: Option<&'a str>,
    ) -> Self {
        Self {
            request_id,
            request,
            shared,
            excluded_profiles,
            route_kind,
            selection_started_at,
            continuation,
            wait_affinity_owner,
        }
    }
}

pub(super) fn runtime_proxy_maybe_wait_for_interactive_inflight_relief(
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
    let wait_budget = runtime_proxy_request_inflight_wait_budget(request, pressure_mode);
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
        format!(
            "request={request_id} transport=http inflight_wait_started route={} wait_ms={}",
            runtime_route_kind_label(route_kind),
            total_wait_budget.as_millis()
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
        format!(
            "request={request_id} transport=http inflight_wait_finished route={} waited_ms={} signaled={signaled} useful={} wake_source={}",
            runtime_route_kind_label(route_kind),
            started_at.elapsed().as_millis(),
            useful_relief,
            runtime_profile_inflight_wait_outcome_label(wake_source),
        ),
    );
    Ok(useful_relief)
}
