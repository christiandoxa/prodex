use super::*;

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
    match band {
        RuntimeQuotaPressureBand::Healthy => "quota_healthy",
        RuntimeQuotaPressureBand::Thin => "quota_thin",
        RuntimeQuotaPressureBand::Critical => "quota_critical",
        RuntimeQuotaPressureBand::Exhausted => "quota_exhausted",
        RuntimeQuotaPressureBand::Unknown => "quota_unknown",
    }
}

pub(crate) fn runtime_quota_window_status_reason(status: RuntimeQuotaWindowStatus) -> &'static str {
    match status {
        RuntimeQuotaWindowStatus::Ready => "ready",
        RuntimeQuotaWindowStatus::Thin => "thin",
        RuntimeQuotaWindowStatus::Critical => "critical",
        RuntimeQuotaWindowStatus::Exhausted => "exhausted",
        RuntimeQuotaWindowStatus::Unknown => "unknown",
    }
}

pub(crate) fn runtime_quota_window_summary(
    usage: &UsageResponse,
    label: &str,
) -> RuntimeQuotaWindowSummary {
    let Some(window) = required_main_window_snapshot(usage, label) else {
        return RuntimeQuotaWindowSummary {
            status: RuntimeQuotaWindowStatus::Unknown,
            remaining_percent: 0,
            reset_at: i64::MAX,
        };
    };
    let status = if window.remaining_percent == 0 {
        RuntimeQuotaWindowStatus::Exhausted
    } else if window.remaining_percent <= 5 {
        RuntimeQuotaWindowStatus::Critical
    } else if window.remaining_percent <= 15 {
        RuntimeQuotaWindowStatus::Thin
    } else {
        RuntimeQuotaWindowStatus::Ready
    };
    RuntimeQuotaWindowSummary {
        status,
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

pub(crate) fn runtime_quota_summary_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    RuntimeQuotaSummary {
        five_hour: runtime_quota_window_summary(usage, "5h"),
        weekly: runtime_quota_window_summary(usage, "weekly"),
        route_band: runtime_quota_pressure_band_for_route(usage, route_kind),
    }
}

pub(crate) fn runtime_quota_summary_blocking_reset_at(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<i64> {
    let floor_percent = runtime_quota_precommit_floor_percent(route_kind);
    [summary.five_hour, summary.weekly]
        .into_iter()
        .filter(|window| runtime_quota_window_precommit_guard(*window, floor_percent))
        .map(|window| window.reset_at)
        .filter(|reset_at| *reset_at != i64::MAX)
        .max()
}

pub(crate) fn runtime_profile_usage_snapshot_from_usage(
    usage: &UsageResponse,
) -> RuntimeProfileUsageSnapshot {
    let five_hour = runtime_quota_window_summary(usage, "5h");
    let weekly = runtime_quota_window_summary(usage, "weekly");
    RuntimeProfileUsageSnapshot {
        checked_at: Local::now().timestamp(),
        five_hour_status: five_hour.status,
        five_hour_remaining_percent: five_hour.remaining_percent,
        five_hour_reset_at: five_hour.reset_at,
        weekly_status: weekly.status,
        weekly_remaining_percent: weekly.remaining_percent,
        weekly_reset_at: weekly.reset_at,
    }
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
    let five_hour = runtime_quota_window_summary_from_usage_snapshot_at(
        snapshot.five_hour_status,
        snapshot.five_hour_remaining_percent,
        snapshot.five_hour_reset_at,
        now,
    );
    let weekly = runtime_quota_window_summary_from_usage_snapshot_at(
        snapshot.weekly_status,
        snapshot.weekly_remaining_percent,
        snapshot.weekly_reset_at,
        now,
    );
    let route_band = [
        five_hour.status,
        weekly.status,
        match route_kind {
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => weekly.status,
            RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => five_hour.status,
        },
    ]
    .into_iter()
    .fold(RuntimeQuotaPressureBand::Healthy, |band, status| {
        band.max(match status {
            RuntimeQuotaWindowStatus::Ready => RuntimeQuotaPressureBand::Healthy,
            RuntimeQuotaWindowStatus::Thin => RuntimeQuotaPressureBand::Thin,
            RuntimeQuotaWindowStatus::Critical => RuntimeQuotaPressureBand::Critical,
            RuntimeQuotaWindowStatus::Exhausted => RuntimeQuotaPressureBand::Exhausted,
            RuntimeQuotaWindowStatus::Unknown => RuntimeQuotaPressureBand::Unknown,
        })
    });
    RuntimeQuotaSummary {
        five_hour,
        weekly,
        route_band,
    }
}

pub(crate) fn runtime_quota_window_summary_from_usage_snapshot_at(
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> RuntimeQuotaWindowSummary {
    if reset_at != i64::MAX && reset_at <= now {
        return RuntimeQuotaWindowSummary {
            status: RuntimeQuotaWindowStatus::Ready,
            remaining_percent: 100,
            reset_at,
        };
    }
    RuntimeQuotaWindowSummary {
        status,
        remaining_percent,
        reset_at,
    }
}

pub(crate) fn runtime_profile_usage_snapshot_hold_active(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at > now
    })
}

pub(crate) fn runtime_profile_usage_snapshot_hold_expired(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at <= now
    })
}

pub(crate) fn runtime_proxy_quota_reset_at_from_message(message: &str) -> Option<i64> {
    let marker = message.to_ascii_lowercase().find("try again at ")?;
    let candidate = message
        .get(marker + "try again at ".len()..)?
        .trim()
        .trim_end_matches('.');
    let now = Local::now();
    if let Some((time_text, meridiem)) = candidate
        .split_whitespace()
        .collect::<Vec<_>>()
        .get(..2)
        .and_then(|parts| {
            if parts.len() == 2 {
                Some((parts[0], parts[1]))
            } else {
                None
            }
        })
        && let Ok(time) =
            chrono::NaiveTime::parse_from_str(&format!("{time_text} {meridiem}"), "%I:%M %p")
    {
        let mut naive = now.date_naive().and_time(time);
        let mut parsed = Local
            .from_local_datetime(&naive)
            .single()
            .or_else(|| Local.from_local_datetime(&naive).earliest())?;
        if parsed.timestamp() <= now.timestamp() {
            naive = naive.checked_add_signed(chrono::Duration::days(1))?;
            parsed = Local
                .from_local_datetime(&naive)
                .single()
                .or_else(|| Local.from_local_datetime(&naive).earliest())?;
        }
        return Some(parsed.timestamp());
    }
    let mut parts = candidate
        .split_whitespace()
        .map(|part| part.to_string())
        .collect::<Vec<_>>();
    if parts.len() < 5 {
        return None;
    }
    let day_digits = parts[1]
        .trim_end_matches(',')
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if day_digits.is_empty() {
        return None;
    }
    parts[1] = format!("{day_digits},");
    let normalized = parts[..5].join(" ");
    let naive = chrono::NaiveDateTime::parse_from_str(&normalized, "%b %d, %Y %I:%M %p").ok()?;
    Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .map(|datetime| datetime.timestamp())
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

pub(crate) fn usage_from_runtime_usage_snapshot(
    snapshot: &RuntimeProfileUsageSnapshot,
) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.five_hour_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.five_hour_reset_at != i64::MAX)
                    .then_some(snapshot.five_hour_reset_at),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.weekly_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.weekly_reset_at != i64::MAX)
                    .then_some(snapshot.weekly_reset_at),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

pub(crate) fn runtime_quota_source_label(source: RuntimeQuotaSource) -> &'static str {
    match source {
        RuntimeQuotaSource::LiveProbe => "probe_cache",
        RuntimeQuotaSource::PersistedSnapshot => "persisted_snapshot",
    }
}

pub(crate) fn runtime_usage_snapshot_is_usable(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    if runtime_profile_usage_snapshot_hold_active(snapshot, now) {
        return true;
    }
    if runtime_profile_usage_snapshot_hold_expired(snapshot, now) {
        return false;
    }
    now.saturating_sub(snapshot.checked_at) <= RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS
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
    let score = ready_profile_score_for_route(usage, route_kind);
    (
        runtime_quota_pressure_band_for_route(usage, route_kind),
        score.total_pressure,
        score.weekly_pressure,
        score.five_hour_pressure,
        Reverse(score.reserve_floor),
        Reverse(score.weekly_remaining),
        Reverse(score.five_hour_remaining),
        score.weekly_reset_at,
        score.five_hour_reset_at,
    )
}

pub(crate) fn runtime_quota_pressure_sort_key_for_route_from_summary(
    summary: RuntimeQuotaSummary,
) -> RuntimeQuotaPressureSortKey {
    (
        summary.route_band,
        match summary.route_band {
            RuntimeQuotaPressureBand::Healthy => 0,
            RuntimeQuotaPressureBand::Thin => 1,
            RuntimeQuotaPressureBand::Critical => 2,
            RuntimeQuotaPressureBand::Exhausted => 3,
            RuntimeQuotaPressureBand::Unknown => 4,
        },
        match summary.weekly.status {
            RuntimeQuotaWindowStatus::Ready => 0,
            RuntimeQuotaWindowStatus::Thin => 1,
            RuntimeQuotaWindowStatus::Critical => 2,
            RuntimeQuotaWindowStatus::Exhausted => 3,
            RuntimeQuotaWindowStatus::Unknown => 4,
        },
        match summary.five_hour.status {
            RuntimeQuotaWindowStatus::Ready => 0,
            RuntimeQuotaWindowStatus::Thin => 1,
            RuntimeQuotaWindowStatus::Critical => 2,
            RuntimeQuotaWindowStatus::Exhausted => 3,
            RuntimeQuotaWindowStatus::Unknown => 4,
        },
        Reverse(
            summary
                .weekly
                .remaining_percent
                .min(summary.five_hour.remaining_percent),
        ),
        Reverse(summary.weekly.remaining_percent),
        Reverse(summary.five_hour.remaining_percent),
        summary.weekly.reset_at,
        summary.five_hour.reset_at,
    )
}

pub(crate) fn runtime_quota_pressure_band_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureBand {
    let Some(weekly) = required_main_window_snapshot(usage, "weekly") else {
        return RuntimeQuotaPressureBand::Unknown;
    };
    let Some(five_hour) = required_main_window_snapshot(usage, "5h") else {
        return RuntimeQuotaPressureBand::Unknown;
    };

    let weekly_remaining = weekly.remaining_percent;
    let five_hour_remaining = five_hour.remaining_percent;
    if weekly_remaining == 0 || five_hour_remaining == 0 {
        return RuntimeQuotaPressureBand::Exhausted;
    }

    let (thin_weekly, thin_five_hour, critical_weekly, critical_five_hour) = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => (20, 10, 10, 5),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => (10, 5, 5, 3),
    };

    if weekly_remaining <= critical_weekly || five_hour_remaining <= critical_five_hour {
        RuntimeQuotaPressureBand::Critical
    } else if weekly_remaining <= thin_weekly || five_hour_remaining <= thin_five_hour {
        RuntimeQuotaPressureBand::Thin
    } else {
        RuntimeQuotaPressureBand::Healthy
    }
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
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && !matches!(source, Some(RuntimeQuotaSource::LiveProbe))
        && (matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Critical)
            || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Critical)
            || matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
            || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown))
}

pub(crate) fn runtime_quota_summary_requires_live_source_after_probe(
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimePrecommitQuotaBlockReason {
    ExhaustedBeforeSend,
    CriticalFloorBeforeSend,
    WindowsUnavailableAfterReprobe,
}

impl RuntimePrecommitQuotaBlockReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RuntimePrecommitQuotaBlockReason::ExhaustedBeforeSend => "quota_exhausted_before_send",
            RuntimePrecommitQuotaBlockReason::CriticalFloorBeforeSend => {
                "quota_critical_floor_before_send"
            }
            RuntimePrecommitQuotaBlockReason::WindowsUnavailableAfterReprobe => {
                "quota_windows_unavailable_after_reprobe"
            }
        }
    }
}

fn runtime_precommit_quota_block_reason(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<RuntimePrecommitQuotaBlockReason> {
    let floor_percent = runtime_quota_precommit_floor_percent(route_kind);
    if summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        return Some(RuntimePrecommitQuotaBlockReason::ExhaustedBeforeSend);
    }

    if matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && (runtime_quota_window_precommit_guard(summary.five_hour, floor_percent)
        || runtime_quota_window_precommit_guard(summary.weekly, floor_percent))
    {
        return Some(RuntimePrecommitQuotaBlockReason::CriticalFloorBeforeSend);
    }

    None
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

pub(crate) fn runtime_quota_source_sort_key(
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
