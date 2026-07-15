//! Compact-route retryable quota and overload handling.

use std::collections::BTreeSet;
use std::time::Instant;

use super::super::super::{
    RuntimeAutoRedeemResetCreditOutcome, bump_runtime_profile_bad_pairing_score,
    bump_runtime_profile_health_score, mark_runtime_profile_retry_backoff,
    release_runtime_compact_lineage, release_runtime_quota_blocked_affinity,
    runtime_auto_redeem_usage_limit_reset_credit, runtime_has_route_eligible_quota_fallback,
    runtime_proxy_log,
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
    pub(super) request_session_id: Option<&'a str>,
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
        request_session_id,
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

    if !overload
        && !auto_redeemed_profiles.contains(&profile_name)
        && runtime_auto_redeem_usage_limit_reset_credit(
            shared,
            &profile_name,
            RuntimeRouteKind::Compact,
            "compact_quota_blocked",
            false,
        )? == RuntimeAutoRedeemResetCreditOutcome::Redeemed
    {
        auto_redeemed_profiles.insert(profile_name);
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http quota_blocked_auto_redeemed_retry route=compact"
            ),
        );
        return Ok(RuntimeCompactFailureFlow::Retry);
    }

    let mut released_affinity = false;
    let mut released_compact_lineage = false;
    if !overload {
        released_affinity = release_runtime_quota_blocked_affinity(
            shared,
            &profile_name,
            None,
            None,
            request_session_id,
        )?;
        released_compact_lineage = release_runtime_compact_lineage(
            shared,
            &profile_name,
            request_session_id,
            None,
            "quota_blocked",
        )?;
        if session_profile.as_deref() == Some(profile_name.as_str()) {
            *session_profile = None;
        }
        if compact_followup_profile
            .as_ref()
            .is_some_and(|(owner, _)| owner == &profile_name)
        {
            *compact_followup_profile = None;
        }
    }

    let should_retry_same_profile = overload
        && !conservative_overload_retried_profiles.contains(&profile_name)
        && (compact_followup_profile
            .as_ref()
            .is_some_and(|(owner, _)| owner == &profile_name)
            || session_profile.as_deref() == Some(profile_name.as_str())
            || current_profile == profile_name);
    if should_retry_same_profile {
        conservative_overload_retried_profiles.insert(profile_name.clone());
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_overload_conservative_retry profile={profile_name} delay_ms={RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS} reason=non_blocking_retry"
            ),
        );
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
    mark_runtime_profile_retry_backoff(shared, &profile_name)?;

    if !overload
        && runtime_compact_candidate_has_hard_affinity(
            &profile_name,
            compact_followup_profile
                .as_ref()
                .map(|(profile_name, _)| profile_name.as_str()),
            session_profile.as_deref(),
        )
    {
        log_runtime_proxy_compact_attempt_final_failure(
            shared,
            RuntimeProxyCompactAttemptFailureLog {
                request_id,
                exit: "hard_affinity_retryable_failure",
                reason: "quota",
                selection_attempts,
                selection_started_at,
                pressure_mode,
                last_failure: last_failure.as_ref(),
                saw_inflight_saturation,
                saw_transport_failure,
                profile_name: &profile_name,
            },
        );
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
        let _ = bump_runtime_profile_health_score(
            shared,
            &profile_name,
            RuntimeRouteKind::Compact,
            RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
            "compact_overload",
        );
        let _ = bump_runtime_profile_bad_pairing_score(
            shared,
            &profile_name,
            RuntimeRouteKind::Compact,
            RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
            "compact_overload",
        );
    }
    if !overload
        && !runtime_has_route_eligible_quota_fallback(
            shared,
            &profile_name,
            &BTreeSet::new(),
            RuntimeRouteKind::Compact,
        )?
    {
        log_runtime_proxy_compact_attempt_final_failure(
            shared,
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
        );
        return Ok(RuntimeCompactFailureFlow::Return(response));
    }

    excluded_profiles.insert(profile_name);
    *last_failure = Some((response, !overload));
    Ok(RuntimeCompactFailureFlow::Retry)
}
