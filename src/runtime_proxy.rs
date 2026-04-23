use super::*;

mod admission;
mod affinity;
mod attempt_outcome;
mod buffered_response;
mod chain_log;
mod classification;
mod continuation;
mod dispatch;
mod failure_response;
mod health;
mod lifecycle;
mod lineage;
mod path;
mod payload_detection;
mod prefetch;
mod previous_response_log;
mod previous_response_orchestration;
mod profile_state;
mod quota;
mod response_forwarding;
mod selection;
mod selection_plan;
mod standard;
mod transport_failure;
mod upstream;
mod websocket;
mod websocket_message;

pub(crate) use self::admission::*;
pub(crate) use self::affinity::*;
pub(crate) use self::attempt_outcome::*;
pub(super) use self::buffered_response::*;
pub(crate) use self::chain_log::*;
pub(crate) use self::classification::*;
pub(super) use self::continuation::*;
pub(crate) use self::dispatch::*;
pub(crate) use self::failure_response::*;
pub(super) use self::health::*;
pub(crate) use self::lifecycle::*;
pub(super) use self::lineage::*;
pub(super) use self::path::*;
pub(super) use self::payload_detection::*;
pub(super) use self::prefetch::*;
pub(crate) use self::previous_response_log::*;
pub(crate) use self::previous_response_orchestration::*;
pub(super) use self::profile_state::*;
pub(super) use self::quota::*;
pub(crate) use self::response_forwarding::*;
pub(crate) use self::selection::*;
pub(crate) use self::selection_plan::*;
pub(crate) use self::standard::*;
pub(super) use self::transport_failure::*;
pub(crate) use self::upstream::*;
pub(crate) use self::websocket::*;
use self::websocket_message::*;

pub(super) fn runtime_previous_response_retry_delay(retry_index: usize) -> Option<Duration> {
    RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS
        .get(retry_index)
        .copied()
        .map(Duration::from_millis)
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
#[allow(dead_code)]
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
    let fallback_profiles = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_profile_selection_catalog(&runtime)
            .entries
            .into_iter()
            .filter(|profile| {
                profile.name != profile_name && !excluded_profiles.contains(&profile.name)
            })
            .map(|profile| {
                let cached_auth_summary =
                    runtime_profile_cached_auth_summary_from_maps_for_selection(
                        &profile.name,
                        &runtime.profile_usage_auth,
                        &runtime.profile_probe_cache,
                    );
                let auth_failure_active = runtime_profile_auth_failure_active_with_auth_cache(
                    &runtime.profile_health,
                    &runtime.profile_usage_auth,
                    &profile.name,
                    now,
                );
                let in_selection_backoff = runtime_profile_name_in_selection_backoff(
                    &profile.name,
                    &runtime.profile_retry_backoff_until,
                    &runtime.profile_transport_backoff_until,
                    &runtime.profile_route_circuit_open_until,
                    route_kind,
                    now,
                );
                let inflight_count =
                    runtime_profile_inflight_sort_key(&profile.name, &runtime.profile_inflight);

                (
                    profile.name,
                    profile.codex_home,
                    cached_auth_summary,
                    auth_failure_active,
                    in_selection_backoff,
                    inflight_count,
                )
            })
            .collect::<Vec<_>>()
    };

    for (
        _candidate_name,
        codex_home,
        cached_auth_summary,
        auth_failure_active,
        in_selection_backoff,
        inflight_count,
    ) in fallback_profiles
    {
        if auth_failure_active || in_selection_backoff || inflight_count >= inflight_soft_limit {
            continue;
        }

        let quota_compatible = cached_auth_summary
            .or_else(|| allow_disk_auth_fallback.then(|| read_auth_summary(&codex_home)))
            .is_some_and(|summary| summary.quota_compatible);
        if quota_compatible {
            return Ok(true);
        }
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
    let mut committed_response_ids = BTreeSet::new();
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
                        Some(Duration::from_millis(
                            runtime_proxy_websocket_precommit_progress_timeout_ms(),
                        )),
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

                if !committed && inspected.precommit_hold {
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
                    for frame in &buffered_precommit_text_frames {
                        committed_response_ids.extend(frame.response_ids.iter().cloned());
                    }
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

                committed_response_ids.extend(inspected.response_ids.iter().cloned());
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
                let committed_previous_response_not_found = committed
                    && matches!(
                        inspected.retry_kind,
                        Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound)
                    );
                if committed_previous_response_not_found {
                    let mut dead_response_ids =
                        committed_response_ids.iter().cloned().collect::<Vec<_>>();
                    if let Some(previous_response_id) = request_previous_response_id {
                        dead_response_ids.push(previous_response_id.to_string());
                    }
                    let _ = clear_runtime_dead_response_bindings(
                        shared,
                        profile_name,
                        &dead_response_ids,
                        "previous_response_not_found_after_commit",
                    );
                    runtime_proxy_log_previous_response_stale_continuation(
                        shared,
                        RuntimePreviousResponseLogContext {
                            request_id,
                            transport: "websocket",
                            route: "websocket",
                            websocket_session: None,
                            via: None,
                        },
                        profile_name,
                    );
                    runtime_proxy_log_chain_dead_upstream_confirmed(
                        shared,
                        RuntimeProxyChainLog {
                            request_id,
                            transport: "websocket",
                            route: "websocket",
                            websocket_session: None,
                            profile_name,
                            previous_response_id: request_previous_response_id,
                            reason: "previous_response_not_found_locked_affinity",
                            via: None,
                        },
                        Some("post_commit"),
                    );
                }
                let text = if committed_previous_response_not_found {
                    runtime_translate_precommit_previous_response_websocket_text_frame(&text)
                } else {
                    runtime_translate_previous_response_websocket_text_frame(&text)
                };
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
                    if committed_previous_response_not_found {
                        let _ = upstream_socket.close(None);
                        websocket_session.reset();
                    } else {
                        websocket_session.store(
                            upstream_socket,
                            profile_name,
                            upstream_turn_state,
                            inflight_guard.take(),
                        );
                    }
                    return Ok(RuntimeWebsocketAttempt::Delivered);
                }
            }
            Ok(WsMessage::Binary(payload)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(
                            runtime_proxy_websocket_precommit_progress_timeout_ms(),
                        )),
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
                        Some(Duration::from_millis(
                            runtime_proxy_websocket_precommit_progress_timeout_ms(),
                        )),
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
                        Some(Duration::from_millis(
                            runtime_proxy_websocket_precommit_progress_timeout_ms(),
                        )),
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
                if !committed && precommit_hold_count > 0 && runtime_websocket_timeout_error(&err) {
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
                    if reuse_existing_session {
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
                    return Err(transport_error);
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
    let request = request.clone();
    let request_requires_previous_response_affinity =
        runtime_request_requires_previous_response_affinity(&request);
    let previous_response_fresh_fallback_shape =
        runtime_request_previous_response_fresh_fallback_shape(&request);
    let previous_response_id = runtime_request_previous_response_id(&request);
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
    let selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    let previous_response_fresh_fallback_used = false;
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
    macro_rules! runtime_responses_previous_response_not_found_context {
        ($profile_name:expr, $turn_state:expr, $via:expr, $policy:expr $(,)?) => {
            RuntimePreviousResponseNotFoundContext {
                shared,
                log_context: RuntimePreviousResponseLogContext {
                    request_id,
                    transport: "http",
                    route: "responses",
                    websocket_session: None,
                    via: $via,
                },
                route: RuntimePreviousResponseNotFoundRoute::Responses,
                route_kind: RuntimeRouteKind::Responses,
                profile_name: $profile_name,
                turn_state: $turn_state,
                previous_response_id: previous_response_id.as_deref(),
                request_turn_state: request_turn_state.as_deref(),
                request_session_id: request_session_id.as_deref(),
                request_requires_previous_response_affinity,
                trusted_previous_response_affinity,
                previous_response_fresh_fallback_used,
                fresh_fallback_shape: previous_response_fresh_fallback_shape,
                policy: $policy,
            }
        };
    }
    macro_rules! runtime_responses_previous_response_not_found_state {
        ($trusted_previous_response_affinity:expr $(,)?) => {
            RuntimePreviousResponseNotFoundState {
                saw_previous_response_not_found: &mut saw_previous_response_not_found,
                previous_response_retry_candidate: &mut previous_response_retry_candidate,
                previous_response_retry_index: &mut previous_response_retry_index,
                candidate_turn_state_retry_profile: &mut candidate_turn_state_retry_profile,
                candidate_turn_state_retry_value: &mut candidate_turn_state_retry_value,
                bound_profile: &mut bound_profile,
                session_profile: &mut session_profile,
                pinned_profile: &mut pinned_profile,
                turn_state_profile: &mut turn_state_profile,
                compact_followup_profile: Some(&mut compact_followup_profile),
                excluded_profiles: &mut excluded_profiles,
                trusted_previous_response_affinity: $trusted_previous_response_affinity,
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
                            previous_response_fresh_fallback_shape,
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
                        match handle_runtime_previous_response_not_found(
                            runtime_responses_previous_response_not_found_context!(
                                &profile_name,
                                turn_state,
                                Some("direct_current_profile_fallback"),
                                RuntimePreviousResponseNotFoundPolicy::responses(false),
                            ),
                            runtime_responses_previous_response_not_found_state!(None),
                        )? {
                            RuntimePreviousResponseNotFoundAction::RetryOwner
                            | RuntimePreviousResponseNotFoundAction::Rotate => {
                                last_failure =
                                    Some((RuntimeUpstreamFailureResponse::Http(response), false));
                                continue;
                            }
                            RuntimePreviousResponseNotFoundAction::StaleContinuation => {
                                unreachable!(
                                    "responses previous_response policy cannot return this action"
                                )
                            }
                        }
                    }
                    RuntimeResponsesAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        match runtime_responses_local_selection_action(
                            runtime_quota_blocked_affinity_is_releasable(
                                runtime_candidate_affinity!(
                                    RuntimeRouteKind::Responses,
                                    &profile_name,
                                    compact_followup_profile
                                        .as_ref()
                                        .map(|(profile_name, _)| profile_name.as_str()),
                                ),
                                request_requires_previous_response_affinity,
                                previous_response_fresh_fallback_shape,
                            ),
                        ) {
                            RuntimeResponsesLocalSelectionAction::ReturnServiceUnavailable => {
                                return Ok(runtime_responses_local_selection_failure_reply());
                            }
                            RuntimeResponsesLocalSelectionAction::Rotate => {}
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
                            previous_response_fresh_fallback_shape,
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
                        match handle_runtime_previous_response_not_found(
                            runtime_responses_previous_response_not_found_context!(
                                &profile_name,
                                turn_state,
                                Some("direct_current_profile_fallback"),
                                RuntimePreviousResponseNotFoundPolicy::responses(false),
                            ),
                            runtime_responses_previous_response_not_found_state!(None),
                        )? {
                            RuntimePreviousResponseNotFoundAction::RetryOwner
                            | RuntimePreviousResponseNotFoundAction::Rotate => {
                                last_failure =
                                    Some((RuntimeUpstreamFailureResponse::Http(response), false));
                                continue;
                            }
                            RuntimePreviousResponseNotFoundAction::StaleContinuation => {
                                unreachable!(
                                    "responses previous_response policy cannot return this action"
                                )
                            }
                        }
                    }
                    RuntimeResponsesAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        match runtime_responses_local_selection_action(
                            runtime_quota_blocked_affinity_is_releasable(
                                runtime_candidate_affinity!(
                                    RuntimeRouteKind::Responses,
                                    &profile_name,
                                    compact_followup_profile
                                        .as_ref()
                                        .map(|(profile_name, _)| profile_name.as_str()),
                                ),
                                request_requires_previous_response_affinity,
                                previous_response_fresh_fallback_shape,
                            ),
                        ) {
                            RuntimeResponsesLocalSelectionAction::ReturnServiceUnavailable => {
                                return Ok(runtime_responses_local_selection_failure_reply());
                            }
                            RuntimeResponsesLocalSelectionAction::Rotate => {}
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
                    previous_response_fresh_fallback_shape,
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
                match runtime_responses_local_selection_action(
                    runtime_quota_blocked_affinity_is_releasable(
                        runtime_candidate_affinity!(
                            RuntimeRouteKind::Responses,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                        ),
                        request_requires_previous_response_affinity,
                        previous_response_fresh_fallback_shape,
                    ),
                ) {
                    RuntimeResponsesLocalSelectionAction::ReturnServiceUnavailable => {
                        return Ok(runtime_responses_local_selection_failure_reply());
                    }
                    RuntimeResponsesLocalSelectionAction::Rotate => {}
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
                excluded_profiles.insert(profile_name);
            }
            RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name,
                response,
                turn_state,
            } => {
                match handle_runtime_previous_response_not_found(
                    runtime_responses_previous_response_not_found_context!(
                        &profile_name,
                        turn_state,
                        None,
                        RuntimePreviousResponseNotFoundPolicy::responses(true),
                    ),
                    runtime_responses_previous_response_not_found_state!(Some(
                        &mut trusted_previous_response_affinity
                    )),
                )? {
                    RuntimePreviousResponseNotFoundAction::RetryOwner
                    | RuntimePreviousResponseNotFoundAction::Rotate => {
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Http(response), false));
                        continue;
                    }
                    RuntimePreviousResponseNotFoundAction::StaleContinuation => {
                        unreachable!("responses previous_response policy cannot return this action")
                    }
                }
            }
        }
    }
}

enum RuntimeResponsesLocalSelectionAction {
    ReturnServiceUnavailable,
    Rotate,
}

fn runtime_responses_local_selection_action(
    releasable: bool,
) -> RuntimeResponsesLocalSelectionAction {
    if releasable {
        RuntimeResponsesLocalSelectionAction::Rotate
    } else {
        RuntimeResponsesLocalSelectionAction::ReturnServiceUnavailable
    }
}

fn runtime_responses_local_selection_failure_reply() -> RuntimeResponsesReply {
    RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
        503,
        "service_unavailable",
        runtime_proxy_local_selection_failure_message(),
    ))
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
    let mut recovery_steps = RuntimeProfileUnauthorizedRecoveryStep::ordered();
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
        let prepared = prepare_runtime_proxy_responses_success(
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
        return prepared;
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
    let mut selection_state = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_route_selection_catalog(&runtime, route_kind, now)
    };
    let probe_plan = build_runtime_response_probe_plan(
        &selection_state,
        excluded_profiles,
        route_kind,
        allow_disk_auth_fallback,
        sync_probe_pressure_mode,
        inflight_soft_limit,
        now,
    );
    for refresh in &probe_plan.stale_probe_refreshes {
        schedule_runtime_probe_refresh(shared, &refresh.name, &refresh.codex_home);
    }
    if let Some(skip_jobs) = probe_plan.sync_probe_skip_jobs_count {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_sync_probe route={} reason=pressure_mode cold_start_jobs={}",
                runtime_route_kind_label(route_kind),
                skip_jobs,
            ),
        );
    }

    let mut reports = probe_plan.reports;
    let mut ready_candidates = probe_plan.ready_candidates;
    if probe_plan.should_sync_probe_cold_start {
        let base_url = Some(selection_state.upstream_base_url.clone());
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
        selection_state = {
            let mut runtime = shared
                .runtime
                .lock()
                .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
            prune_runtime_profile_selection_backoff(&mut runtime, now);
            runtime_route_selection_catalog(&runtime, route_kind, now)
        };
        let cached_usage_snapshots = selection_state.persisted_usage_snapshots();

        for fresh_report in fresh_reports {
            if let Some(existing) = reports
                .iter_mut()
                .find(|report| report.name == fresh_report.name)
            {
                *existing = fresh_report;
            }
        }
        reports.sort_by_key(|report| report.order_index);
        ready_candidates = ready_profile_candidates_with_view(
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
    } else {
        if let Some(skip_profiles) = probe_plan.sync_probe_skip_profiles_count {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_sync_probe route={} reason=pressure_mode cold_start_profiles={}",
                    runtime_route_kind_label(route_kind),
                    skip_profiles
                ),
            );
        }
        for job in probe_plan.cold_start_probe_jobs {
            schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
        }
    }
    let candidate_plan = build_runtime_response_candidate_execution_plan(
        &selection_state,
        excluded_profiles,
        route_kind,
        inflight_soft_limit,
        ready_candidates,
        |name| runtime_profile_selection_jitter(shared, name, route_kind),
    );

    for candidate in candidate_plan.ready_candidates {
        if let Some(reason) = candidate.ready_skip_reason() {
            if reason == "profile_inflight_soft_limit" {
                runtime_proxy_log(
                    shared,
                    format!(
                        "selection_skip_current route={} profile={} reason=profile_inflight_soft_limit inflight={} soft_limit={} health={} quota_source={} {}",
                        runtime_route_kind_label(route_kind),
                        candidate.name,
                        candidate.inflight_count,
                        candidate.inflight_soft_limit,
                        candidate.health_sort_key,
                        runtime_quota_source_label(candidate.quota_source),
                        runtime_quota_summary_log_fields(candidate.quota_summary),
                    ),
                );
            } else {
                runtime_proxy_log(
                    shared,
                    format!(
                        "selection_skip_current route={} profile={} reason={} inflight={} health={} quota_source={} {}",
                        runtime_route_kind_label(route_kind),
                        candidate.name,
                        reason,
                        candidate.inflight_count,
                        candidate.health_sort_key,
                        runtime_quota_source_label(candidate.quota_source),
                        runtime_quota_summary_log_fields(candidate.quota_summary),
                    ),
                );
            }
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
                    candidate.inflight_count,
                    candidate.health_sort_key,
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(candidate.quota_summary),
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
                candidate.inflight_count,
                candidate.health_sort_key,
                candidate.order_index,
                format_args!(
                    "quota_source={} {}",
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(candidate.quota_summary)
                ),
            ),
        );
        return Ok(Some(candidate.name.clone()));
    }

    let mut fallback = None;
    for candidate in candidate_plan.fallback_candidates {
        if let Some(reason) = candidate.fallback_skip_reason() {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason={} inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    reason,
                    candidate.inflight_count,
                    candidate.health_sort_key,
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(candidate.quota_summary),
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
                    candidate.inflight_count,
                    candidate.health_sort_key,
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(candidate.quota_summary),
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
                candidate.inflight_count,
                candidate.health_sort_key,
                candidate.backoff_sort_key,
                candidate.order_index,
                runtime_quota_source_label(candidate.quota_source),
                runtime_quota_summary_log_fields(candidate.quota_summary),
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
    let now = Local::now().timestamp();
    let state = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        runtime_route_selection_catalog(&runtime, route_kind, now)
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

pub(super) fn runtime_waitable_inflight_candidates_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    wait_affinity_owner: Option<&str>,
) -> Result<BTreeSet<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let state = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_route_selection_catalog(&runtime, route_kind, now)
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
    let state = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_route_selection_catalog(&runtime, route_kind, now)
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
    #[allow(clippy::too_many_arguments)]
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
