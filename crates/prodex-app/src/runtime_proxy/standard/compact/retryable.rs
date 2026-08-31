//! Compact-route retryable quota and overload handling.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use super::super::super::{
    RuntimeAutoRedeemResetCreditOutcome, await_runtime_proxy_async_task,
    bump_runtime_profile_bad_pairing_score, bump_runtime_profile_health_score,
    mark_runtime_profile_retry_backoff, release_runtime_compact_lineage,
    release_runtime_quota_blocked_affinity, runtime_auto_redeem_usage_limit_reset_credit,
    runtime_has_route_eligible_quota_fallback_for_model,
    runtime_luna_quota_block_has_spark_capacity, runtime_proxy_log,
};
use super::{
    affinity::runtime_compact_candidate_has_hard_affinity,
    flow::RuntimeCompactFailureFlow,
    logging::{
        RuntimeProxyCompactAttemptFailureLog, log_runtime_proxy_compact_attempt_final_failure,
    },
};
use crate::core_constants::{
    RUNTIME_PROFILE_BAD_PAIRING_PENALTY, RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
    RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS,
};
use crate::runtime_state_shared::{RuntimeRotationProxyShared, RuntimeRouteKind};
use anyhow::Result;

pub(super) struct RuntimeProxyCompactRetryableFailure<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: String,
    pub(super) response: tiny_http::ResponseBox,
    pub(super) overload: bool,
    pub(super) previous_response_profile: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) request_model_name: Option<&'a str>,
    pub(super) current_profile: &'a str,
    pub(super) compact_followup_profile: &'a mut Option<(String, &'static str)>,
    pub(super) session_profile: &'a mut Option<String>,
    pub(super) auto_redeemed_profiles: &'a mut BTreeSet<String>,
    pub(super) conservative_overload_retried_profiles: &'a mut BTreeSet<String>,
    pub(super) excluded_profiles: &'a mut BTreeSet<String>,
    pub(super) last_failure: &'a mut Option<(tiny_http::ResponseBox, bool)>,
    pub(super) selection_attempts: usize,
    pub(super) selection_started_at: Instant,
    pub(super) pressure_mode: bool,
    pub(super) saw_inflight_saturation: bool,
    pub(super) saw_transport_failure: bool,
}

pub(super) fn handle_runtime_proxy_compact_retryable_failure(
    failure: RuntimeProxyCompactRetryableFailure<'_>,
) -> Result<RuntimeCompactFailureFlow> {
    let RuntimeProxyCompactRetryableFailure {
        request_id,
        shared,
        profile_name,
        response,
        overload,
        previous_response_profile,
        request_session_id,
        request_turn_state,
        request_model_name,
        current_profile,
        compact_followup_profile,
        session_profile,
        auto_redeemed_profiles,
        conservative_overload_retried_profiles,
        excluded_profiles,
        last_failure,
        selection_attempts,
        selection_started_at,
        pressure_mode,
        saw_inflight_saturation,
        saw_transport_failure,
    } = failure;

    if runtime_compact_try_auto_redeem(
        request_id,
        shared,
        &profile_name,
        overload,
        auto_redeemed_profiles,
    )? {
        return Ok(RuntimeCompactFailureFlow::Retry);
    }

    if runtime_compact_try_conservative_overload_retry(
        request_id,
        shared,
        &profile_name,
        overload,
        current_profile,
        RuntimeCompactAffinityOwners {
            compact_followup_profile: compact_followup_profile
                .as_ref()
                .map(|(profile_name, _)| profile_name.as_str()),
            previous_response_profile,
            session_profile: session_profile.as_deref(),
        },
        conservative_overload_retried_profiles,
    )? {
        *last_failure = Some((response, false));
        return Ok(RuntimeCompactFailureFlow::Retry);
    }

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http compact_retryable_failure profile={profile_name} reason={}",
            if overload { "overload" } else { "quota" }
        ),
    );
    if !(prodex_quota::openai_model_is_luna(request_model_name)
        && !overload
        && runtime_luna_quota_block_has_spark_capacity(shared, &profile_name))
    {
        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
    }

    if runtime_compact_quota_fallback_exhausted(
        shared,
        overload,
        request_model_name,
        RuntimeProxyCompactAttemptFailureLog {
            request_id,
            exit: "quota_fallback_exhausted",
            reason: "quota",
            selection_attempts,
            selection_started_at,
            pressure_mode,
            last_failure: last_failure.as_ref(),
            saw_inflight_saturation,
            saw_transport_failure,
            profile_name: &profile_name,
        },
    )? {
        return Ok(RuntimeCompactFailureFlow::Return(response));
    }

    if runtime_compact_previous_profile_hard_affinity_failure(
        shared,
        &profile_name,
        previous_response_profile,
        RuntimeCompactAffinityOwners {
            compact_followup_profile: compact_followup_profile
                .as_ref()
                .map(|(profile_name, _)| profile_name.as_str()),
            previous_response_profile,
            session_profile: session_profile.as_deref(),
        },
        RuntimeProxyCompactAttemptFailureLog {
            request_id,
            exit: "hard_affinity_retryable_failure",
            reason: if overload { "overload" } else { "quota" },
            selection_attempts,
            selection_started_at,
            pressure_mode,
            last_failure: last_failure.as_ref(),
            saw_inflight_saturation,
            saw_transport_failure,
            profile_name: &profile_name,
        },
    ) {
        return Ok(RuntimeCompactFailureFlow::Return(response));
    }

    let (released_affinity, released_compact_lineage) = release_runtime_compact_quota_state(
        shared,
        &profile_name,
        overload,
        request_session_id,
        request_turn_state,
        compact_followup_profile,
        session_profile,
    )?;

    if runtime_compact_hard_affinity_failure(
        shared,
        RuntimeCompactAffinityOwners {
            compact_followup_profile: compact_followup_profile
                .as_ref()
                .map(|(profile_name, _)| profile_name.as_str()),
            previous_response_profile,
            session_profile: session_profile.as_deref(),
        },
        RuntimeProxyCompactAttemptFailureLog {
            request_id,
            exit: "hard_affinity_retryable_failure",
            reason: if overload { "overload" } else { "quota" },
            selection_attempts,
            selection_started_at,
            pressure_mode,
            last_failure: last_failure.as_ref(),
            saw_inflight_saturation,
            saw_transport_failure,
            profile_name: &profile_name,
        },
    ) {
        return Ok(RuntimeCompactFailureFlow::Return(response));
    }

    if released_affinity {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} route=compact"
            ),
        );
    }
    if released_compact_lineage {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_lineage_released profile={profile_name} reason=quota_blocked"
            ),
        );
    }
    if overload {
        runtime_compact_record_overload_penalty(shared, &profile_name);
    }

    excluded_profiles.insert(profile_name);
    *last_failure = Some((response, !overload));
    Ok(RuntimeCompactFailureFlow::Retry)
}

struct RuntimeCompactAffinityOwners<'a> {
    compact_followup_profile: Option<&'a str>,
    previous_response_profile: Option<&'a str>,
    session_profile: Option<&'a str>,
}

fn runtime_compact_previous_profile_hard_affinity_failure(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_profile: Option<&str>,
    owners: RuntimeCompactAffinityOwners<'_>,
    failure_log: RuntimeProxyCompactAttemptFailureLog<'_>,
) -> bool {
    previous_response_profile == Some(profile_name)
        && runtime_compact_hard_affinity_failure(shared, owners, failure_log)
}

fn runtime_compact_try_auto_redeem(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    overload: bool,
    auto_redeemed_profiles: &mut BTreeSet<String>,
) -> Result<bool> {
    if overload || auto_redeemed_profiles.contains(profile_name) {
        return Ok(false);
    }
    if runtime_auto_redeem_usage_limit_reset_credit(
        shared,
        profile_name,
        RuntimeRouteKind::Compact,
        "compact_quota_blocked",
        false,
    )? != RuntimeAutoRedeemResetCreditOutcome::Redeemed
    {
        return Ok(false);
    }
    auto_redeemed_profiles.insert(profile_name.to_string());
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http quota_blocked_auto_redeemed_retry route=compact"
        ),
    );
    Ok(true)
}

fn release_runtime_compact_quota_state(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    overload: bool,
    request_session_id: Option<&str>,
    request_turn_state: Option<&str>,
    compact_followup_profile: &mut Option<(String, &'static str)>,
    session_profile: &mut Option<String>,
) -> Result<(bool, bool)> {
    if overload {
        return Ok((false, false));
    }
    let released_turn_state_affinity = release_runtime_quota_blocked_affinity(
        shared,
        profile_name,
        None,
        request_turn_state,
        None,
    )?;
    let released_session_affinity = release_runtime_quota_blocked_affinity(
        shared,
        profile_name,
        None,
        None,
        request_session_id,
    )?;
    let released_compact_lineage = release_runtime_compact_lineage(
        shared,
        profile_name,
        request_session_id,
        request_turn_state,
        "quota_blocked",
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
    Ok((
        released_turn_state_affinity || released_session_affinity,
        released_compact_lineage,
    ))
}

fn runtime_compact_try_conservative_overload_retry(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    overload: bool,
    current_profile: &str,
    owners: RuntimeCompactAffinityOwners<'_>,
    retried_profiles: &mut BTreeSet<String>,
) -> Result<bool> {
    let owner_match = owners.compact_followup_profile == Some(profile_name)
        || owners.previous_response_profile == Some(profile_name)
        || owners.session_profile == Some(profile_name)
        || current_profile == profile_name;
    if !overload || retried_profiles.contains(profile_name) || !owner_match {
        return Ok(false);
    }
    await_runtime_proxy_async_task(shared, "compact_overload_retry_delay", async {
        tokio::time::sleep(Duration::from_millis(
            RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS,
        ))
        .await;
        Ok(())
    })?;
    retried_profiles.insert(profile_name.to_string());
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http compact_overload_conservative_retry profile={profile_name} delay_ms={RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS} reason=non_blocking_retry"
        ),
    );
    Ok(true)
}

fn runtime_compact_hard_affinity_failure(
    shared: &RuntimeRotationProxyShared,
    owners: RuntimeCompactAffinityOwners<'_>,
    failure: RuntimeProxyCompactAttemptFailureLog<'_>,
) -> bool {
    if !runtime_compact_candidate_has_hard_affinity(
        failure.profile_name,
        owners.compact_followup_profile,
        owners.previous_response_profile,
        owners.session_profile,
    ) {
        return false;
    }
    log_runtime_proxy_compact_attempt_final_failure(shared, failure);
    true
}

fn runtime_compact_record_overload_penalty(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) {
    let _ = bump_runtime_profile_health_score(
        shared,
        profile_name,
        RuntimeRouteKind::Compact,
        RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
        "compact_overload",
    );
    let _ = bump_runtime_profile_bad_pairing_score(
        shared,
        profile_name,
        RuntimeRouteKind::Compact,
        RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
        "compact_overload",
    );
}

fn runtime_compact_quota_fallback_exhausted(
    shared: &RuntimeRotationProxyShared,
    overload: bool,
    request_model_name: Option<&str>,
    failure: RuntimeProxyCompactAttemptFailureLog<'_>,
) -> Result<bool> {
    if overload
        || runtime_has_route_eligible_quota_fallback_for_model(
            shared,
            failure.profile_name,
            &BTreeSet::new(),
            RuntimeRouteKind::Compact,
            request_model_name,
        )?
        || (prodex_quota::openai_model_is_luna(request_model_name)
            && runtime_luna_quota_block_has_spark_capacity(shared, failure.profile_name))
    {
        return Ok(false);
    }
    log_runtime_proxy_compact_attempt_final_failure(shared, failure);
    Ok(true)
}
