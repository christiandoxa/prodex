use super::*;

pub(super) fn wait_for_compact_overload_recovery(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &mut BTreeSet<String>,
    recovery_sweeps: &mut usize,
    recovery_started_at: &mut Option<Instant>,
) -> Result<bool> {
    let recovery_started_at = *recovery_started_at.get_or_insert_with(Instant::now);
    if recovery_started_at.elapsed()
        >= Duration::from_millis(runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS)
    {
        return Ok(false);
    }
    let profile_count = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .state
        .profiles
        .len();
    if profile_count < 2 {
        return Ok(false);
    }
    let recovered = clear_runtime_recovered_profiles(
        shared,
        excluded_profiles,
        RuntimeRouteKind::Compact,
        true,
    )?;
    if recovered > 0 {
        *recovery_sweeps = recovery_sweeps.saturating_add(1);
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http rotation_sweep_start route=compact recovered_profiles={recovered} sweep={recovery_sweeps}"
            ),
        );
        return Ok(true);
    }
    let Some(until) =
        runtime_profile_recovery_wait_for_route(shared, RuntimeRouteKind::Compact, true)?
    else {
        return Ok(false);
    };
    let now = chrono::Local::now().timestamp();
    let remaining_budget =
        Duration::from_millis(runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS)
            .saturating_sub(recovery_started_at.elapsed());
    let wait = Duration::from_secs(u64::try_from(until.saturating_sub(now)).unwrap_or(0))
        .saturating_add(Duration::from_secs(1))
        .min(remaining_budget);
    if wait.is_zero() {
        return Ok(false);
    }
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http rotation_waiting_for_recovery route=compact wait_ms={} sweep={}",
            wait.as_millis(),
            recovery_sweeps.saturating_add(1)
        ),
    );
    await_runtime_proxy_async_task(shared, "profile_recovery_wait", async move {
        tokio::time::sleep(wait).await;
        Ok(())
    })?;
    let recovered = clear_runtime_recovered_profiles(
        shared,
        excluded_profiles,
        RuntimeRouteKind::Compact,
        true,
    )?;
    *recovery_sweeps = recovery_sweeps.saturating_add(1);
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http rotation_sweep_start route=compact recovered_profiles={recovered} sweep={recovery_sweeps}"
        ),
    );
    Ok(recovered > 0)
}
