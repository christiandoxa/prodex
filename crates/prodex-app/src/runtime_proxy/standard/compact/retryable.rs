//! Compact-route retryable quota and overload handling.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use super::super::super::{
    RuntimeAutoRedeemResetCreditOutcome, await_runtime_proxy_async_task,
    bump_runtime_profile_bad_pairing_score, bump_runtime_profile_health_score,
    mark_runtime_profile_retry_backoff, release_runtime_compact_lineage,
    release_runtime_quota_blocked_affinity, runtime_auto_redeem_usage_limit_reset_credit,
    runtime_has_route_eligible_quota_fallback_for_model, runtime_proxy_log,
};
use super::{
    affinity::runtime_compact_candidate_has_hard_affinity,
    flow::RuntimeCompactFailureFlow,
    logging::{
        RuntimeCompactFailureKind, RuntimeCompactLastFailure, RuntimeProxyCompactAttemptFailureLog,
        log_runtime_proxy_compact_attempt_final_failure,
    },
    recovery::{
        RuntimeCompactHardAffinityRecovery, RuntimeCompactHardAffinityRecoveryRequest,
        recover_runtime_compact_hard_affinity,
    },
};
use crate::core_constants::{
    RUNTIME_PROFILE_BAD_PAIRING_PENALTY, RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
    RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS,
};
use crate::runtime_state_shared::{RuntimeRotationProxyShared, RuntimeRouteKind};
use anyhow::Result;
use prodex_mojo_core::runtime::{
    CompactRetryAction, CompactRetryDecisionInput, CompactRetryReason, CompactRetryStage,
};

pub(super) struct RuntimeProxyCompactRetryableFailure<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: String,
    pub(super) response: tiny_http::ResponseBox,
    pub(super) overload: bool,
    pub(super) previous_response_profile: Option<&'a str>,
    pub(super) request_previous_response_id: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) request_model_name: Option<&'a str>,
    pub(super) current_profile: &'a str,
    pub(super) compact_followup_profile: &'a mut Option<(String, &'static str)>,
    pub(super) session_profile: &'a mut Option<String>,
    pub(super) auto_redeemed_profiles: &'a mut BTreeSet<String>,
    pub(super) conservative_overload_retried_profiles: &'a mut BTreeSet<String>,
    pub(super) excluded_profiles: &'a mut BTreeSet<String>,
    pub(super) last_failure: &'a mut Option<RuntimeCompactLastFailure>,
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
        request_previous_response_id,
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
    let mut stage = CompactRetryStage::Start;
    let mut quota_fallback_available = None;
    let mut released_affinity = false;
    let mut released_compact_lineage = false;

    loop {
        let hard_affinity = runtime_compact_candidate_has_hard_affinity(
            &profile_name,
            compact_followup_profile
                .as_ref()
                .map(|(profile_name, _)| profile_name.as_str()),
            previous_response_profile,
            session_profile.as_deref(),
        );
        let decision =
            prodex_mojo_core::runtime::compact_retry_decision(CompactRetryDecisionInput {
                stage,
                overload,
                auto_redeemed: auto_redeemed_profiles.contains(&profile_name),
                owner_retry_used: conservative_overload_retried_profiles.contains(&profile_name),
                quota_fallback_available,
                hard_affinity,
                committed: false,
                candidate_profile: &profile_name,
                current_profile,
                compact_followup_profile: compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                previous_response_profile,
                session_profile: session_profile.as_deref(),
            })
            .expect("Mojo compact retry decision returned invalid output");

        match decision.action {
            CompactRetryAction::TryAutoRedeem => {
                if runtime_compact_try_auto_redeem(
                    request_id,
                    shared,
                    &profile_name,
                    request_model_name,
                    auto_redeemed_profiles,
                )? {
                    return Ok(RuntimeCompactFailureFlow::Retry);
                }
                stage = CompactRetryStage::AfterAutoRedeem;
            }
            CompactRetryAction::RetryOwnerOverload => {
                runtime_compact_try_conservative_overload_retry(
                    request_id,
                    shared,
                    &profile_name,
                    conservative_overload_retried_profiles,
                )?;
                *last_failure = Some((response, RuntimeCompactFailureKind::Overload));
                return Ok(RuntimeCompactFailureFlow::Retry);
            }
            CompactRetryAction::MarkRetryBackoff => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_retryable_failure profile={profile_name} reason={}",
                        decision.reason.label()
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                stage = CompactRetryStage::AfterBackoff;
            }
            CompactRetryAction::CheckQuotaFallback => {
                quota_fallback_available = Some(runtime_compact_has_quota_fallback(
                    shared,
                    &profile_name,
                    request_model_name,
                    excluded_profiles,
                )?);
                stage = CompactRetryStage::AfterQuotaFallback;
            }
            CompactRetryAction::ReturnQuotaExhausted => {
                log_runtime_proxy_compact_attempt_final_failure(
                    shared,
                    RuntimeProxyCompactAttemptFailureLog {
                        request_id,
                        exit: "quota_fallback_exhausted",
                        reason: decision.reason.label(),
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
            CompactRetryAction::RecoverHardAffinity => {
                quota_fallback_available = None;
                match runtime_compact_retryable_full_context_recovery(
                    RuntimeCompactRetryableRecoveryContext {
                        request_id,
                        shared,
                        profile_name: &profile_name,
                        reason: decision.reason.recovery_label(),
                        previous_response_profile,
                        request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        request_model_name,
                        compact_followup_profile,
                        session_profile,
                        excluded_profiles,
                    },
                )? {
                    RuntimeCompactHardAffinityRecovery::Return(retry) => {
                        return Ok(RuntimeCompactFailureFlow::Return(retry));
                    }
                    RuntimeCompactHardAffinityRecovery::Retry
                    | RuntimeCompactHardAffinityRecovery::Unchanged => {}
                }
                stage = CompactRetryStage::AfterAffinityRecovery;
            }
            CompactRetryAction::ReleaseQuotaState => {
                quota_fallback_available = None;
                (released_affinity, released_compact_lineage) =
                    release_runtime_compact_quota_state(
                        shared,
                        &profile_name,
                        request_session_id,
                        request_turn_state,
                        compact_followup_profile,
                        session_profile,
                    )?;
                stage = CompactRetryStage::AfterAffinityRelease;
            }
            CompactRetryAction::ReturnAffinityFailure => {
                log_runtime_proxy_compact_attempt_final_failure(
                    shared,
                    RuntimeProxyCompactAttemptFailureLog {
                        request_id,
                        exit: "hard_affinity_retryable_failure",
                        reason: decision.reason.label(),
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
            CompactRetryAction::RotateQuota | CompactRetryAction::RotateOverload => {
                return Ok(finish_runtime_compact_retryable_failure(
                    RuntimeCompactRetryableFailureFinalization {
                        request_id,
                        shared,
                        profile_name,
                        response,
                        reason: decision.reason,
                        released_affinity,
                        released_compact_lineage,
                        excluded_profiles,
                        last_failure,
                    },
                ));
            }
            CompactRetryAction::ReturnCommitted => {
                return Ok(RuntimeCompactFailureFlow::Return(response));
            }
        }
    }
}

struct RuntimeCompactRetryableRecoveryContext<'a> {
    request_id: u64,
    shared: &'a RuntimeRotationProxyShared,
    profile_name: &'a str,
    reason: &'static str,
    previous_response_profile: Option<&'a str>,
    request_previous_response_id: Option<&'a str>,
    request_session_id: Option<&'a str>,
    request_turn_state: Option<&'a str>,
    request_model_name: Option<&'a str>,
    compact_followup_profile: &'a mut Option<(String, &'static str)>,
    session_profile: &'a mut Option<String>,
    excluded_profiles: &'a BTreeSet<String>,
}

fn runtime_compact_retryable_full_context_recovery(
    context: RuntimeCompactRetryableRecoveryContext<'_>,
) -> Result<RuntimeCompactHardAffinityRecovery> {
    let RuntimeCompactRetryableRecoveryContext {
        request_id,
        shared,
        profile_name,
        reason,
        previous_response_profile,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
        request_model_name,
        compact_followup_profile,
        session_profile,
        excluded_profiles,
    } = context;
    recover_runtime_compact_hard_affinity(RuntimeCompactHardAffinityRecoveryRequest {
        request_id,
        shared,
        profile_name,
        hard_affinity: true,
        previous_response_profile,
        previous_response_id: request_previous_response_id,
        request_session_id,
        request_turn_state,
        request_model_name,
        compact_followup_profile,
        session_profile,
        excluded_profiles,
        reason,
    })
}

struct RuntimeCompactRetryableFailureFinalization<'a> {
    request_id: u64,
    shared: &'a RuntimeRotationProxyShared,
    profile_name: String,
    response: tiny_http::ResponseBox,
    reason: CompactRetryReason,
    released_affinity: bool,
    released_compact_lineage: bool,
    excluded_profiles: &'a mut BTreeSet<String>,
    last_failure: &'a mut Option<RuntimeCompactLastFailure>,
}

fn finish_runtime_compact_retryable_failure(
    finalization: RuntimeCompactRetryableFailureFinalization<'_>,
) -> RuntimeCompactFailureFlow {
    let RuntimeCompactRetryableFailureFinalization {
        request_id,
        shared,
        profile_name,
        response,
        reason,
        released_affinity,
        released_compact_lineage,
        excluded_profiles,
        last_failure,
    } = finalization;
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
    if reason == CompactRetryReason::Overload {
        runtime_compact_record_overload_penalty(shared, &profile_name);
    }

    excluded_profiles.insert(profile_name);
    *last_failure = Some((
        response,
        match reason {
            CompactRetryReason::Overload => RuntimeCompactFailureKind::Overload,
            CompactRetryReason::Quota => RuntimeCompactFailureKind::Quota,
        },
    ));
    RuntimeCompactFailureFlow::Retry
}

fn runtime_compact_try_auto_redeem(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    request_model_name: Option<&str>,
    auto_redeemed_profiles: &mut BTreeSet<String>,
) -> Result<bool> {
    if runtime_auto_redeem_usage_limit_reset_credit(
        shared,
        profile_name,
        RuntimeRouteKind::Compact,
        request_model_name,
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
    request_session_id: Option<&str>,
    request_turn_state: Option<&str>,
    compact_followup_profile: &mut Option<(String, &'static str)>,
    session_profile: &mut Option<String>,
) -> Result<(bool, bool)> {
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
    retried_profiles: &mut BTreeSet<String>,
) -> Result<()> {
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
    Ok(())
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

fn runtime_compact_has_quota_fallback(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    request_model_name: Option<&str>,
    excluded_profiles: &BTreeSet<String>,
) -> Result<bool> {
    runtime_has_route_eligible_quota_fallback_for_model(
        shared,
        profile_name,
        excluded_profiles,
        RuntimeRouteKind::Compact,
        request_model_name,
    )
}

#[cfg(test)]
pub(crate) fn test_runtime_compact_quota_fallback_exhausted(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    excluded_profiles: &BTreeSet<String>,
    requested_model: Option<&str>,
) -> Result<bool> {
    let quota_fallback_available = runtime_compact_has_quota_fallback(
        shared,
        profile_name,
        requested_model,
        excluded_profiles,
    )?;
    Ok(
        prodex_mojo_core::runtime::compact_retry_decision(CompactRetryDecisionInput {
            stage: CompactRetryStage::AfterQuotaFallback,
            overload: false,
            auto_redeemed: false,
            owner_retry_used: false,
            quota_fallback_available: Some(quota_fallback_available),
            hard_affinity: false,
            committed: false,
            candidate_profile: profile_name,
            current_profile: "",
            compact_followup_profile: None,
            previous_response_profile: None,
            session_profile: None,
        })
        .expect("Mojo compact retry decision returned invalid output")
        .action
            == CompactRetryAction::ReturnQuotaExhausted,
    )
}
