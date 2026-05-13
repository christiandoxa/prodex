use super::*;

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
