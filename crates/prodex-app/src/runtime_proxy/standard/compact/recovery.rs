use super::super::super::{
    build_runtime_proxy_json_error_response, release_runtime_compact_lineage,
    release_runtime_retryable_failure_affinity,
    runtime_has_route_eligible_quota_fallback_for_model,
    runtime_quota_last_chance_profile_for_route,
};
use super::{
    RuntimeInflightReliefWait, RuntimeInflightReliefWaitResult, RuntimeRotationProxyShared,
    RuntimeRouteKind, await_runtime_proxy_async_task, clear_runtime_recovered_profiles,
    runtime_profile_recovery_wait_for_route, runtime_proxy_log,
    runtime_proxy_maybe_wait_for_interactive_inflight_relief, runtime_route_has_retryable_profile,
};
use anyhow::Result;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

pub(super) enum RuntimeCompactHardAffinityRecovery {
    Unchanged,
    Retry,
    Return(tiny_http::ResponseBox),
}

pub(super) struct RuntimeCompactHardAffinityRecoveryRequest<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: &'a str,
    pub(super) hard_affinity: bool,
    pub(super) previous_response_profile: Option<&'a str>,
    pub(super) previous_response_id: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) request_model_name: Option<&'a str>,
    pub(super) compact_followup_profile: &'a mut Option<(String, &'static str)>,
    pub(super) session_profile: &'a mut Option<String>,
    pub(super) excluded_profiles: &'a BTreeSet<String>,
    pub(super) reason: &'static str,
}

fn runtime_compact_retry_fallback_available(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    excluded_profiles: &BTreeSet<String>,
    request_model_name: Option<&str>,
) -> Result<bool> {
    let mut excluded = excluded_profiles.clone();
    excluded.insert(profile_name.to_string());
    if runtime_has_route_eligible_quota_fallback_for_model(
        shared,
        profile_name,
        &excluded,
        RuntimeRouteKind::Compact,
        request_model_name,
    )? {
        return Ok(true);
    }
    Ok(runtime_quota_last_chance_profile_for_route(
        shared,
        &excluded,
        RuntimeRouteKind::Compact,
        None,
        request_model_name,
    )?
    .is_some())
}

pub(super) fn recover_runtime_compact_hard_affinity(
    recovery: RuntimeCompactHardAffinityRecoveryRequest<'_>,
) -> Result<RuntimeCompactHardAffinityRecovery> {
    let RuntimeCompactHardAffinityRecoveryRequest {
        request_id,
        shared,
        profile_name,
        hard_affinity,
        previous_response_profile,
        previous_response_id,
        request_session_id,
        request_turn_state,
        request_model_name,
        compact_followup_profile,
        session_profile,
        excluded_profiles,
        reason,
    } = recovery;
    if !hard_affinity
        || !runtime_compact_retry_fallback_available(
            shared,
            profile_name,
            excluded_profiles,
            request_model_name,
        )?
    {
        return Ok(RuntimeCompactHardAffinityRecovery::Unchanged);
    }

    let previous_response_owner = previous_response_profile == Some(profile_name);
    if previous_response_owner {
        if !runtime_proxy_crate::runtime_full_context_retry_signal_eligible(
            previous_response_id.is_some(),
            request_session_id.is_some(),
            true,
        ) {
            return Ok(RuntimeCompactHardAffinityRecovery::Unchanged);
        }
        let released_affinity = release_runtime_retryable_failure_affinity(
            shared,
            profile_name,
            previous_response_id,
            request_turn_state,
            request_session_id,
            "compact_full_context_retry",
        )?;
        let released_lineage = release_runtime_compact_lineage(
            shared,
            profile_name,
            request_session_id,
            request_turn_state,
            "compact_full_context_retry",
        )?;
        if session_profile.as_deref() == Some(profile_name) {
            *session_profile = None;
        }
        if compact_followup_profile
            .as_ref()
            .is_some_and(|(owner, _)| owner == profile_name)
        {
            *compact_followup_profile = None;
        }
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_full_context_retry_signal profile={profile_name} reason={reason} affinity_released={released_affinity} lineage_released={released_lineage}"
            ),
        );
        return Ok(RuntimeCompactHardAffinityRecovery::Return(
            build_runtime_proxy_json_error_response(
                400,
                "previous_response_not_found",
                "Previous response was not found. Retrying the full request.",
            ),
        ));
    }

    let released_affinity = release_runtime_retryable_failure_affinity(
        shared,
        profile_name,
        None,
        request_turn_state,
        request_session_id,
        reason,
    )?;
    let released_lineage = release_runtime_compact_lineage(
        shared,
        profile_name,
        request_session_id,
        request_turn_state,
        reason,
    )?;
    if session_profile.as_deref() == Some(profile_name) {
        *session_profile = None;
    }
    if compact_followup_profile
        .as_ref()
        .is_some_and(|(owner, _)| owner == profile_name)
    {
        *compact_followup_profile = None;
    }
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http compact_affinity_recovered profile={profile_name} reason={reason} affinity_released={released_affinity} lineage_released={released_lineage}"
        ),
    );
    Ok(RuntimeCompactHardAffinityRecovery::Retry)
}

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
    selection_started_at: &mut Instant,
    observed_release_revision: Option<u64>,
    continuation: bool,
    wait_affinity_owner: Option<&str>,
) -> Result<RuntimeInflightReliefWaitResult> {
    runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
        observed_release_revision,
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
    force_reselection_after_wait: bool,
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
    Ok(recovered > 0 || force_reselection_after_wait)
}
