use super::*;

pub(super) fn wait_for_compact_overload_recovery(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &mut BTreeSet<String>,
    recovery_sweeps: &mut usize,
    recovery_started_at: &mut Option<Instant>,
    provider_outage: bool,
) -> Result<bool> {
    if !provider_outage
        && *recovery_sweeps >= runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_SWEEP_LIMIT
    {
        return Ok(false);
    }
    let recovery_started_at = *recovery_started_at.get_or_insert_with(Instant::now);
    if !provider_outage
        && recovery_started_at.elapsed()
            >= Duration::from_millis(
                runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS,
            )
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
    if profile_count < 2 && !provider_outage {
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
        if provider_outage {
            let exponent = (*recovery_sweeps).min(5) as u32;
            let wait = Duration::from_millis(
                250_u64
                    .saturating_mul(1_u64 << exponent)
                    .saturating_add((request_id.saturating_add(*recovery_sweeps as u64)) % 251)
                    .min(30_000),
            );
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http provider_temporarily_unavailable_retry route=compact wait_ms={} sweep={}",
                    wait.as_millis(),
                    recovery_sweeps.saturating_add(1)
                ),
            );
            await_runtime_proxy_async_task(shared, "provider_recovery_wait", async move {
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
            return Ok(recovered > 0 || provider_outage);
        }
        return Ok(false);
    };
    let now = chrono::Local::now().timestamp();
    let remaining_budget = if provider_outage {
        Duration::from_secs(30)
    } else {
        Duration::from_millis(runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS)
            .saturating_sub(recovery_started_at.elapsed())
    };
    let wait = Duration::from_secs(u64::try_from(until.saturating_sub(now)).unwrap_or(0))
        .saturating_add(Duration::from_secs(1))
        .min(remaining_budget);
    let wait = if wait.is_zero() && provider_outage {
        Duration::from_millis(
            250_u64
                .saturating_mul(1_u64 << (*recovery_sweeps).min(5) as u32)
                .saturating_add((request_id.saturating_add(*recovery_sweeps as u64)) % 251)
                .min(30_000),
        )
    } else {
        wait
    };
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
