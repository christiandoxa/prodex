use super::{
    RuntimeInflightReliefWait, RuntimeInflightReliefWaitResult, RuntimeRotationProxyShared,
    RuntimeRouteKind, await_runtime_proxy_async_task, clear_runtime_recovered_profiles,
    runtime_profile_recovery_wait_for_route, runtime_proxy_log,
    runtime_proxy_maybe_wait_for_interactive_inflight_relief, runtime_route_has_retryable_profile,
};
use anyhow::Result;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

pub(super) fn compact_profile_count(shared: &RuntimeRotationProxyShared) -> Result<usize> {
    Ok(shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .state
        .profiles
        .len()
        .max(1))
}

pub(super) fn wait_for_compact_inflight_relief(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    selection_started_at: Instant,
    continuation: bool,
    wait_affinity_owner: Option<&str>,
) -> Result<RuntimeInflightReliefWaitResult> {
    runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
        request_id,
        shared,
        excluded_profiles,
        route_kind: RuntimeRouteKind::Compact,
        selection_started_at,
        continuation,
        wait_affinity_owner,
        selected_profile: None,
    })
}

pub(super) fn wait_for_compact_overload_recovery(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &mut BTreeSet<String>,
    recovery_sweeps: &mut usize,
) -> Result<bool> {
    if !runtime_route_has_retryable_profile(shared, RuntimeRouteKind::Compact)? {
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
    let wait = Duration::from_secs(u64::try_from(until.saturating_sub(now)).unwrap_or(0))
        .saturating_add(Duration::from_secs(1))
        .min(Duration::from_secs(30));
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
