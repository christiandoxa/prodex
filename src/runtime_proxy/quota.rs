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
