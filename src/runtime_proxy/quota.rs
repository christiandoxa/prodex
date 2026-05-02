use super::*;

pub(crate) use prodex_quota::quota_reset_at_from_message as runtime_proxy_quota_reset_at_from_message;
pub(crate) use runtime_proxy_crate::RuntimePrecommitQuotaBlockReason;

fn runtime_route_kind_to_proxy(
    route_kind: RuntimeRouteKind,
) -> runtime_proxy_crate::RuntimeRouteKind {
    match route_kind {
        RuntimeRouteKind::Responses => runtime_proxy_crate::RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact => runtime_proxy_crate::RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket => runtime_proxy_crate::RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard => runtime_proxy_crate::RuntimeRouteKind::Standard,
    }
}

fn runtime_quota_source_to_proxy(
    source: RuntimeQuotaSource,
) -> runtime_proxy_crate::RuntimeSelectionQuotaSource {
    match source {
        RuntimeQuotaSource::LiveProbe => {
            runtime_proxy_crate::RuntimeSelectionQuotaSource::LiveProbe
        }
        RuntimeQuotaSource::PersistedSnapshot => {
            runtime_proxy_crate::RuntimeSelectionQuotaSource::PersistedSnapshot
        }
    }
}

fn runtime_quota_source_option_to_proxy(
    source: Option<RuntimeQuotaSource>,
) -> Option<runtime_proxy_crate::RuntimeSelectionQuotaSource> {
    source.map(runtime_quota_source_to_proxy)
}

fn runtime_quota_window_status_to_proxy(
    status: RuntimeQuotaWindowStatus,
) -> runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus {
    match status {
        RuntimeQuotaWindowStatus::Ready => {
            runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus::Ready
        }
        RuntimeQuotaWindowStatus::Thin => {
            runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus::Thin
        }
        RuntimeQuotaWindowStatus::Critical => {
            runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus::Critical
        }
        RuntimeQuotaWindowStatus::Exhausted => {
            runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus::Exhausted
        }
        RuntimeQuotaWindowStatus::Unknown => {
            runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus::Unknown
        }
    }
}

fn runtime_quota_window_status_from_proxy(
    status: runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus,
) -> RuntimeQuotaWindowStatus {
    match status {
        runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus::Ready => {
            RuntimeQuotaWindowStatus::Ready
        }
        runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus::Thin => {
            RuntimeQuotaWindowStatus::Thin
        }
        runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus::Critical => {
            RuntimeQuotaWindowStatus::Critical
        }
        runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus::Exhausted => {
            RuntimeQuotaWindowStatus::Exhausted
        }
        runtime_proxy_crate::RuntimeSelectionQuotaWindowStatus::Unknown => {
            RuntimeQuotaWindowStatus::Unknown
        }
    }
}

fn runtime_quota_pressure_band_to_proxy(
    band: RuntimeQuotaPressureBand,
) -> runtime_proxy_crate::RuntimeSelectionQuotaPressureBand {
    match band {
        RuntimeQuotaPressureBand::Healthy => {
            runtime_proxy_crate::RuntimeSelectionQuotaPressureBand::Healthy
        }
        RuntimeQuotaPressureBand::Thin => {
            runtime_proxy_crate::RuntimeSelectionQuotaPressureBand::Thin
        }
        RuntimeQuotaPressureBand::Critical => {
            runtime_proxy_crate::RuntimeSelectionQuotaPressureBand::Critical
        }
        RuntimeQuotaPressureBand::Exhausted => {
            runtime_proxy_crate::RuntimeSelectionQuotaPressureBand::Exhausted
        }
        RuntimeQuotaPressureBand::Unknown => {
            runtime_proxy_crate::RuntimeSelectionQuotaPressureBand::Unknown
        }
    }
}

fn runtime_quota_pressure_band_from_proxy(
    band: runtime_proxy_crate::RuntimeSelectionQuotaPressureBand,
) -> RuntimeQuotaPressureBand {
    match band {
        runtime_proxy_crate::RuntimeSelectionQuotaPressureBand::Healthy => {
            RuntimeQuotaPressureBand::Healthy
        }
        runtime_proxy_crate::RuntimeSelectionQuotaPressureBand::Thin => {
            RuntimeQuotaPressureBand::Thin
        }
        runtime_proxy_crate::RuntimeSelectionQuotaPressureBand::Critical => {
            RuntimeQuotaPressureBand::Critical
        }
        runtime_proxy_crate::RuntimeSelectionQuotaPressureBand::Exhausted => {
            RuntimeQuotaPressureBand::Exhausted
        }
        runtime_proxy_crate::RuntimeSelectionQuotaPressureBand::Unknown => {
            RuntimeQuotaPressureBand::Unknown
        }
    }
}

fn runtime_quota_window_summary_to_proxy(
    window: RuntimeQuotaWindowSummary,
) -> runtime_proxy_crate::RuntimeProxyQuotaWindowSummary {
    runtime_proxy_crate::RuntimeProxyQuotaWindowSummary {
        status: runtime_quota_window_status_to_proxy(window.status),
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

fn runtime_quota_window_summary_from_proxy(
    window: runtime_proxy_crate::RuntimeProxyQuotaWindowSummary,
) -> RuntimeQuotaWindowSummary {
    RuntimeQuotaWindowSummary {
        status: runtime_quota_window_status_from_proxy(window.status),
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

fn runtime_quota_summary_to_proxy(
    summary: RuntimeQuotaSummary,
) -> runtime_proxy_crate::RuntimeProxyQuotaSummary {
    runtime_proxy_crate::RuntimeProxyQuotaSummary {
        five_hour: runtime_quota_window_summary_to_proxy(summary.five_hour),
        weekly: runtime_quota_window_summary_to_proxy(summary.weekly),
        route_band: runtime_quota_pressure_band_to_proxy(summary.route_band),
    }
}

fn runtime_quota_summary_from_proxy(
    summary: runtime_proxy_crate::RuntimeProxyQuotaSummary,
) -> RuntimeQuotaSummary {
    RuntimeQuotaSummary {
        five_hour: runtime_quota_window_summary_from_proxy(summary.five_hour),
        weekly: runtime_quota_window_summary_from_proxy(summary.weekly),
        route_band: runtime_quota_pressure_band_from_proxy(summary.route_band),
    }
}

fn runtime_usage_snapshot_to_proxy(
    snapshot: &RuntimeProfileUsageSnapshot,
) -> runtime_proxy_crate::RuntimeProxyUsageSnapshot {
    runtime_proxy_crate::RuntimeProxyUsageSnapshot {
        checked_at: snapshot.checked_at,
        five_hour_status: runtime_quota_window_status_to_proxy(snapshot.five_hour_status),
        five_hour_remaining_percent: snapshot.five_hour_remaining_percent,
        five_hour_reset_at: snapshot.five_hour_reset_at,
        weekly_status: runtime_quota_window_status_to_proxy(snapshot.weekly_status),
        weekly_remaining_percent: snapshot.weekly_remaining_percent,
        weekly_reset_at: snapshot.weekly_reset_at,
    }
}

fn runtime_usage_snapshot_from_proxy(
    snapshot: runtime_proxy_crate::RuntimeProxyUsageSnapshot,
) -> RuntimeProfileUsageSnapshot {
    RuntimeProfileUsageSnapshot {
        checked_at: snapshot.checked_at,
        five_hour_status: runtime_quota_window_status_from_proxy(snapshot.five_hour_status),
        five_hour_remaining_percent: snapshot.five_hour_remaining_percent,
        five_hour_reset_at: snapshot.five_hour_reset_at,
        weekly_status: runtime_quota_window_status_from_proxy(snapshot.weekly_status),
        weekly_remaining_percent: snapshot.weekly_remaining_percent,
        weekly_reset_at: snapshot.weekly_reset_at,
    }
}

fn runtime_quota_window_observation(
    usage: &UsageResponse,
    label: &str,
) -> Option<runtime_proxy_crate::RuntimeProxyQuotaWindowObservation> {
    required_main_window_snapshot(usage, label).map(|window| {
        runtime_proxy_crate::RuntimeProxyQuotaWindowObservation {
            remaining_percent: window.remaining_percent,
            reset_at: window.reset_at,
            pressure_score: window.pressure_score,
        }
    })
}

fn runtime_quota_sort_key_from_proxy(
    sort_key: runtime_proxy_crate::RuntimeProxyQuotaPressureSortKey,
) -> RuntimeQuotaPressureSortKey {
    let (
        band,
        total_pressure,
        weekly_pressure,
        five_hour_pressure,
        reserve_floor,
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at,
        five_hour_reset_at,
    ) = sort_key;
    (
        runtime_quota_pressure_band_from_proxy(band),
        total_pressure,
        weekly_pressure,
        five_hour_pressure,
        reserve_floor,
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at,
        five_hour_reset_at,
    )
}

pub(crate) fn runtime_profile_usage_cache_is_fresh(
    entry: &RuntimeProfileProbeCacheEntry,
    now: i64,
) -> bool {
    now.saturating_sub(entry.checked_at) <= RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS
}

pub(crate) fn runtime_profile_probe_cache_freshness(
    entry: &RuntimeProfileProbeCacheEntry,
    now: i64,
) -> RuntimeProbeCacheFreshness {
    let age = now.saturating_sub(entry.checked_at);
    if age <= RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS {
        RuntimeProbeCacheFreshness::Fresh
    } else if age <= RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS {
        RuntimeProbeCacheFreshness::StaleUsable
    } else {
        RuntimeProbeCacheFreshness::Expired
    }
}

pub(crate) fn update_runtime_profile_probe_cache_with_usage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    usage: UsageResponse,
) -> Result<()> {
    let auth = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .state
        .profiles
        .get(profile_name)
        .map(|profile| profile.provider.auth_summary(&profile.codex_home))
        .unwrap_or(AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        });
    apply_runtime_profile_probe_result(shared, profile_name, auth, Ok(usage))
}

pub(crate) fn runtime_quota_pressure_band_reason(band: RuntimeQuotaPressureBand) -> &'static str {
    runtime_proxy_crate::runtime_proxy_quota_pressure_band_reason(
        runtime_quota_pressure_band_to_proxy(band),
    )
}

pub(crate) fn runtime_quota_window_status_reason(status: RuntimeQuotaWindowStatus) -> &'static str {
    runtime_proxy_crate::runtime_proxy_quota_window_status_reason(
        runtime_quota_window_status_to_proxy(status),
    )
}

#[allow(dead_code)]
pub(crate) fn runtime_quota_window_summary(
    usage: &UsageResponse,
    label: &str,
) -> RuntimeQuotaWindowSummary {
    runtime_quota_window_summary_from_proxy(
        runtime_proxy_crate::runtime_proxy_quota_window_summary(runtime_quota_window_observation(
            usage, label,
        )),
    )
}

pub(crate) fn runtime_quota_summary_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    runtime_quota_summary_from_proxy(runtime_proxy_crate::runtime_proxy_quota_summary_for_route(
        runtime_quota_window_observation(usage, "5h"),
        runtime_quota_window_observation(usage, "weekly"),
        runtime_route_kind_to_proxy(route_kind),
    ))
}

pub(crate) fn runtime_quota_summary_blocking_reset_at(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<i64> {
    runtime_proxy_crate::runtime_proxy_quota_summary_blocking_reset_at(
        runtime_quota_summary_to_proxy(summary),
        runtime_route_kind_to_proxy(route_kind),
        runtime_proxy_responses_quota_critical_floor_percent(),
    )
}

pub(crate) fn runtime_profile_usage_snapshot_from_usage(
    usage: &UsageResponse,
) -> RuntimeProfileUsageSnapshot {
    runtime_usage_snapshot_from_proxy(
        runtime_proxy_crate::runtime_proxy_usage_snapshot_from_observations_at(
            runtime_quota_window_observation(usage, "5h"),
            runtime_quota_window_observation(usage, "weekly"),
            Local::now().timestamp(),
        ),
    )
}

pub(crate) fn runtime_quota_summary_from_usage_snapshot(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, Local::now().timestamp())
}

pub(crate) fn runtime_quota_summary_from_usage_snapshot_at(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> RuntimeQuotaSummary {
    runtime_quota_summary_from_proxy(
        runtime_proxy_crate::runtime_proxy_quota_summary_from_usage_snapshot_at(
            runtime_usage_snapshot_to_proxy(snapshot),
            runtime_route_kind_to_proxy(route_kind),
            now,
        ),
    )
}

#[allow(dead_code)]
pub(crate) fn runtime_quota_window_summary_from_usage_snapshot_at(
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> RuntimeQuotaWindowSummary {
    runtime_quota_window_summary_from_proxy(
        runtime_proxy_crate::runtime_proxy_quota_window_summary_from_usage_snapshot_at(
            runtime_quota_window_status_to_proxy(status),
            remaining_percent,
            reset_at,
            now,
        ),
    )
}

#[allow(dead_code)]
pub(crate) fn runtime_profile_usage_snapshot_hold_active(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    runtime_proxy_crate::runtime_proxy_usage_snapshot_hold_active(
        runtime_usage_snapshot_to_proxy(snapshot),
        now,
    )
}

#[allow(dead_code)]
pub(crate) fn runtime_profile_usage_snapshot_hold_expired(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    runtime_proxy_crate::runtime_proxy_usage_snapshot_hold_expired(
        runtime_usage_snapshot_to_proxy(snapshot),
        now,
    )
}

pub(crate) fn runtime_profile_known_quota_reset_at(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Option<i64> {
    let now = Local::now().timestamp();
    runtime
        .profile_probe_cache
        .get(profile_name)
        .and_then(|entry| entry.result.as_ref().ok())
        .map(|usage| runtime_quota_summary_for_route(usage, route_kind))
        .and_then(|summary| runtime_quota_summary_blocking_reset_at(summary, route_kind))
        .filter(|reset_at| *reset_at > now)
        .or_else(|| {
            runtime
                .profile_usage_snapshots
                .get(profile_name)
                .and_then(|snapshot| {
                    runtime_quota_summary_blocking_reset_at(
                        runtime_quota_summary_from_usage_snapshot(snapshot, route_kind),
                        route_kind,
                    )
                })
                .filter(|reset_at| *reset_at > now)
        })
}

pub(crate) fn mark_runtime_profile_quota_quarantine(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    quota_message: Option<&str>,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    let resolved_reset_at = quota_message
        .and_then(runtime_proxy_quota_reset_at_from_message)
        .or_else(|| runtime_profile_known_quota_reset_at(&runtime, profile_name, route_kind))
        .filter(|reset_at| *reset_at > now);
    let until = resolved_reset_at
        .unwrap_or_else(|| now.saturating_add(RUNTIME_PROFILE_QUOTA_QUARANTINE_FALLBACK_SECONDS));
    runtime.profile_probe_cache.remove(profile_name);
    let snapshot = runtime
        .profile_usage_snapshots
        .entry(profile_name.to_string())
        .or_insert(RuntimeProfileUsageSnapshot {
            checked_at: now,
            five_hour_status: RuntimeQuotaWindowStatus::Unknown,
            five_hour_remaining_percent: 0,
            five_hour_reset_at: i64::MAX,
            weekly_status: RuntimeQuotaWindowStatus::Unknown,
            weekly_remaining_percent: 0,
            weekly_reset_at: i64::MAX,
        });
    snapshot.checked_at = now;
    snapshot.five_hour_status = RuntimeQuotaWindowStatus::Exhausted;
    snapshot.five_hour_remaining_percent = 0;
    snapshot.five_hour_reset_at = if snapshot.five_hour_reset_at == i64::MAX {
        until
    } else {
        snapshot.five_hour_reset_at.max(until)
    };
    snapshot.weekly_status = RuntimeQuotaWindowStatus::Exhausted;
    snapshot.weekly_remaining_percent = 0;
    snapshot.weekly_reset_at = if snapshot.weekly_reset_at == i64::MAX {
        until
    } else {
        snapshot.weekly_reset_at.max(until)
    };
    runtime
        .profile_retry_backoff_until
        .entry(profile_name.to_string())
        .and_modify(|current| *current = (*current).max(until))
        .or_insert(until);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("profile_retry_backoff:{profile_name}"),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_quota_quarantine profile={profile_name} route={} until={} reset_at={} message={}",
            runtime_route_kind_label(route_kind),
            until,
            resolved_reset_at.unwrap_or(i64::MAX),
            quota_message.unwrap_or("-"),
        ),
    );
    runtime_proxy_log(
        shared,
        format!("profile_retry_backoff profile={profile_name} until={until}"),
    );
    Ok(())
}

pub(crate) fn runtime_quota_source_label(source: RuntimeQuotaSource) -> &'static str {
    runtime_proxy_crate::runtime_selection_quota_source_label(runtime_quota_source_to_proxy(source))
}

pub(crate) fn runtime_usage_snapshot_is_usable(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    runtime_proxy_crate::runtime_proxy_usage_snapshot_is_usable(
        runtime_usage_snapshot_to_proxy(snapshot),
        now,
        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    )
}

pub(crate) fn runtime_quota_summary_log_fields(summary: RuntimeQuotaSummary) -> String {
    format!(
        "quota_band={} five_hour_status={} five_hour_remaining={} five_hour_reset_at={} weekly_status={} weekly_remaining={} weekly_reset_at={}",
        runtime_quota_pressure_band_reason(summary.route_band),
        runtime_quota_window_status_reason(summary.five_hour.status),
        summary.five_hour.remaining_percent,
        summary.five_hour.reset_at,
        runtime_quota_window_status_reason(summary.weekly.status),
        summary.weekly.remaining_percent,
        summary.weekly.reset_at,
    )
}

pub(crate) type RuntimeQuotaPressureSortKey = (
    RuntimeQuotaPressureBand,
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
);

pub(crate) fn runtime_quota_pressure_sort_key_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureSortKey {
    runtime_quota_sort_key_from_proxy(
        runtime_proxy_crate::runtime_proxy_quota_pressure_sort_key_for_route(
            runtime_quota_window_observation(usage, "5h"),
            runtime_quota_window_observation(usage, "weekly"),
            runtime_route_kind_to_proxy(route_kind),
        ),
    )
}

pub(crate) fn runtime_quota_pressure_sort_key_for_route_from_summary(
    summary: RuntimeQuotaSummary,
) -> RuntimeQuotaPressureSortKey {
    runtime_quota_sort_key_from_proxy(
        runtime_proxy_crate::runtime_proxy_quota_pressure_sort_key_for_route_from_summary(
            runtime_quota_summary_to_proxy(summary),
        ),
    )
}

#[allow(dead_code)]
pub(crate) fn runtime_quota_pressure_band_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureBand {
    runtime_quota_pressure_band_from_proxy(
        runtime_proxy_crate::runtime_proxy_quota_pressure_band_for_route(
            runtime_quota_window_observation(usage, "5h"),
            runtime_quota_window_observation(usage, "weekly"),
            runtime_route_kind_to_proxy(route_kind),
        ),
    )
}

pub(crate) fn runtime_profile_codex_home(
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
pub(crate) fn runtime_has_alternative_quota_compatible_profile(
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

pub(crate) fn runtime_has_route_eligible_quota_fallback(
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

pub(crate) fn refresh_runtime_profile_quota_inline(
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

pub(crate) fn runtime_quota_summary_requires_precommit_live_probe(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime_proxy_crate::runtime_proxy_quota_summary_requires_precommit_live_probe(
        runtime_quota_summary_to_proxy(summary),
        runtime_quota_source_option_to_proxy(source),
        runtime_route_kind_to_proxy(route_kind),
    )
}

pub(crate) fn runtime_quota_summary_requires_live_source_after_probe(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime_proxy_crate::runtime_proxy_quota_summary_requires_live_source_after_probe(
        runtime_quota_summary_to_proxy(summary),
        runtime_quota_source_option_to_proxy(source),
        runtime_route_kind_to_proxy(route_kind),
    )
}

pub(crate) fn ensure_runtime_profile_precommit_quota_ready(
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

pub(crate) struct RuntimePrecommitQuotaGateRequest<'a> {
    pub(crate) shared: &'a RuntimeRotationProxyShared,
    pub(crate) profile_name: &'a str,
    pub(crate) route_kind: RuntimeRouteKind,
    pub(crate) has_continuation_context: bool,
    pub(crate) reprobe_context: &'a str,
}

pub(crate) enum RuntimePrecommitQuotaGateDecision {
    Proceed,
    Block {
        reason: RuntimePrecommitQuotaBlockReason,
        summary: RuntimeQuotaSummary,
        source: Option<RuntimeQuotaSource>,
    },
}

fn runtime_precommit_quota_block_reason(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<RuntimePrecommitQuotaBlockReason> {
    runtime_proxy_crate::runtime_proxy_precommit_quota_block_reason(
        runtime_quota_summary_to_proxy(summary),
        runtime_route_kind_to_proxy(route_kind),
        runtime_proxy_responses_quota_critical_floor_percent(),
    )
}

pub(crate) fn runtime_precommit_quota_gate(
    request: RuntimePrecommitQuotaGateRequest<'_>,
) -> Result<RuntimePrecommitQuotaGateDecision> {
    let RuntimePrecommitQuotaGateRequest {
        shared,
        profile_name,
        route_kind,
        has_continuation_context,
        reprobe_context,
    } = request;

    let (initial_quota_summary, initial_quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
    if has_continuation_context
        && matches!(
            initial_quota_source,
            Some(RuntimeQuotaSource::PersistedSnapshot)
        )
        && let Some(reason) =
            runtime_precommit_quota_block_reason(initial_quota_summary, route_kind)
    {
        return Ok(RuntimePrecommitQuotaGateDecision::Block {
            reason,
            summary: initial_quota_summary,
            source: initial_quota_source,
        });
    }

    let has_alternative_quota_profile = runtime_has_route_eligible_quota_fallback(
        shared,
        profile_name,
        &BTreeSet::new(),
        route_kind,
    )?;
    let (quota_summary, quota_source) = ensure_runtime_profile_precommit_quota_ready(
        shared,
        profile_name,
        route_kind,
        reprobe_context,
    )?;
    if runtime_quota_summary_requires_live_source_after_probe(
        quota_summary,
        quota_source,
        route_kind,
    ) && has_alternative_quota_profile
    {
        return Ok(RuntimePrecommitQuotaGateDecision::Block {
            reason: RuntimePrecommitQuotaBlockReason::WindowsUnavailableAfterReprobe,
            summary: quota_summary,
            source: quota_source,
        });
    }

    if let Some(reason) = runtime_precommit_quota_block_reason(quota_summary, route_kind) {
        return Ok(RuntimePrecommitQuotaGateDecision::Block {
            reason,
            summary: quota_summary,
            source: quota_source,
        });
    }

    Ok(RuntimePrecommitQuotaGateDecision::Proceed)
}

pub(crate) fn runtime_proxy_direct_current_fallback_profile(
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

pub(crate) fn runtime_quota_window_usable_for_auto_rotate(
    status: RuntimeQuotaWindowStatus,
) -> bool {
    matches!(
        status,
        RuntimeQuotaWindowStatus::Ready
            | RuntimeQuotaWindowStatus::Thin
            | RuntimeQuotaWindowStatus::Critical
    )
}

pub(crate) fn runtime_quota_summary_allows_soft_affinity(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    source.is_some()
        && runtime_quota_window_usable_for_auto_rotate(summary.five_hour.status)
        && runtime_quota_window_usable_for_auto_rotate(summary.weekly.status)
        && runtime_quota_precommit_guard_reason(summary, route_kind).is_none()
}

pub(crate) fn runtime_quota_soft_affinity_rejection_reason(
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

pub(crate) fn runtime_profile_quota_summary_for_route(
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

pub(crate) fn runtime_profile_cached_auth_summary_for_selection(
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

pub(crate) fn runtime_profile_cached_auth_summary_from_maps_for_selection(
    profile_name: &str,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
) -> Option<AuthSummary> {
    runtime_profile_cached_auth_summary_for_selection(
        profile_usage_auth.get(profile_name).cloned(),
        profile_probe_cache.get(profile_name).cloned(),
    )
}

pub(crate) fn runtime_snapshot_blocks_same_request_cold_start_probe(
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
