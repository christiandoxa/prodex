use super::super::{
    RuntimeAutoRedeemResetCreditOutcome, RuntimeCandidateAffinity,
    RuntimeResponseCandidateSelection, build_runtime_proxy_json_error_response,
    bump_runtime_profile_bad_pairing_score, bump_runtime_profile_health_score,
    commit_runtime_proxy_profile_selection_with_notice, mark_runtime_profile_retry_backoff,
    release_runtime_auth_failed_affinity, release_runtime_compact_lineage,
    release_runtime_quota_blocked_affinity, runtime_auto_redeem_usage_limit_reset_credit,
    runtime_candidate_has_hard_affinity, runtime_compact_route_followup_bound_profile,
    runtime_has_route_eligible_quota_fallback, runtime_profile_inflight_hard_limited_for_context,
    runtime_proxy_current_profile, runtime_proxy_local_selection_failure_message,
    runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_precommit_budget_exhausted_for_route,
    runtime_proxy_pressure_mode_active_for_route, runtime_proxy_profile_inflight_hard_limit,
    runtime_proxy_should_shed_fresh_compact_request, runtime_proxy_structured_log_message,
    runtime_proxy_sync_probe_pressure_pause,
    runtime_remaining_sync_probe_cold_start_profiles_for_route, runtime_request_session_id,
    runtime_request_turn_state, runtime_session_bound_profile,
    select_runtime_response_candidate_for_route,
};
use super::attempt_runtime_standard_request;
use crate::core_constants::{
    RUNTIME_PROFILE_BAD_PAIRING_PENALTY, RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
    RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS,
};
use crate::runtime_proxy_shared::RuntimeStandardAttempt;
use crate::runtime_state_shared::{RuntimeRotationProxyShared, RuntimeRouteKind};
use crate::shared_types::RuntimeProxyRequest;
use anyhow::Result;
use std::collections::BTreeSet;
use std::time::Instant;
mod fallback;
mod logging;
use fallback::{
    RuntimeProxyCompactSelectionExhausted, finish_runtime_proxy_compact_selection_exhausted,
};
use logging::{
    RuntimeProxyCompactFinalFailureLog, log_runtime_proxy_compact_final_failure,
    runtime_proxy_compact_last_failure_kind,
};

pub(super) fn proxy_runtime_compact_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    let request_session_id = runtime_request_session_id(request);
    let request_turn_state = runtime_request_turn_state(request);
    let mut session_profile = request_session_id
        .as_deref()
        .map(|session_id| runtime_session_bound_profile(shared, session_id))
        .transpose()?
        .flatten();
    let current_profile = runtime_proxy_current_profile(shared)?;
    let mut compact_followup_profile = runtime_compact_route_followup_bound_profile(
        shared,
        request_turn_state.as_deref(),
        request_session_id.as_deref(),
    )?;
    if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_followup_owner profile={profile_name} source={source}"
            ),
        );
    }
    let initial_compact_affinity_profile = compact_followup_profile
        .as_ref()
        .map(|(profile_name, _)| profile_name.as_str())
        .or(session_profile.as_deref());
    let compact_owner_profile = compact_followup_profile
        .as_ref()
        .map(|(profile_name, _)| profile_name.clone())
        .or(session_profile.clone())
        .unwrap_or_else(|| current_profile.clone());
    let pressure_mode =
        runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Compact);
    let selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    if runtime_proxy_should_shed_fresh_compact_request(
        pressure_mode,
        initial_compact_affinity_profile,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_pressure_shed reason=fresh_request pressure_mode={pressure_mode}"
            ),
        );
        log_runtime_proxy_compact_final_failure(
            shared,
            RuntimeProxyCompactFinalFailureLog {
                request_id,
                exit: "pressure",
                reason: "pressure",
                selection_attempts,
                selection_started_at,
                pressure_mode,
                last_failure_kind: "none",
                saw_inflight_saturation: false,
                saw_transport_failure: false,
                profile_name: None,
            },
        );
        return Ok(build_runtime_proxy_json_error_response(
            503,
            "service_unavailable",
            "Fresh compact requests are temporarily deferred while the runtime proxy is under pressure. Retry the request.",
        ));
    }
    let mut excluded_profiles = BTreeSet::new();
    let mut auto_redeemed_profiles = BTreeSet::new();
    let mut conservative_overload_retried_profiles = BTreeSet::new();
    let mut last_failure: Option<(tiny_http::ResponseBox, bool)> = None;
    let mut saw_inflight_saturation = false;
    let mut saw_transport_failure = false;

    loop {
        if runtime_proxy_precommit_budget_exhausted_for_route(
            shared,
            selection_started_at,
            selection_attempts,
            compact_followup_profile.is_some() || session_profile.is_some(),
            pressure_mode,
        )? {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            return finish_runtime_proxy_compact_selection_exhausted(
                RuntimeProxyCompactSelectionExhausted {
                    request_id,
                    request,
                    shared,
                    compact_owner_profile: &compact_owner_profile,
                    strict_affinity_profile: compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    session_profile: session_profile.as_deref(),
                    selection_attempts,
                    selection_started_at,
                    pressure_mode,
                    exit: "precommit_budget_exhausted",
                    fallback_exit: "precommit_budget_exhausted_fallback",
                    saw_transport_failure,
                },
                last_failure,
                saw_inflight_saturation,
            );
        }
        selection_attempts = selection_attempts.saturating_add(1);
        let Some(candidate_name) = select_runtime_response_candidate_for_route(
            shared,
            RuntimeResponseCandidateSelection {
                strict_affinity_profile: compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                session_profile: session_profile.as_deref(),
                ..RuntimeResponseCandidateSelection::fresh(
                    &excluded_profiles,
                    RuntimeRouteKind::Compact,
                )
            },
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_candidate_exhausted last_failure={}",
                    if last_failure.is_some() {
                        "http"
                    } else {
                        "none"
                    }
                ),
            );
            let remaining_cold_start_profiles =
                runtime_remaining_sync_probe_cold_start_profiles_for_route(
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Compact,
                )?;
            if remaining_cold_start_profiles > 0
                && compact_followup_profile.is_none()
                && session_profile.is_none()
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http candidate_exhausted_continue route=compact remaining_cold_start_profiles={remaining_cold_start_profiles}"
                    ),
                );
                runtime_proxy_sync_probe_pressure_pause(shared, RuntimeRouteKind::Compact);
                continue;
            }
            return finish_runtime_proxy_compact_selection_exhausted(
                RuntimeProxyCompactSelectionExhausted {
                    request_id,
                    request,
                    shared,
                    compact_owner_profile: &compact_owner_profile,
                    strict_affinity_profile: compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    session_profile: session_profile.as_deref(),
                    selection_attempts,
                    selection_started_at,
                    pressure_mode,
                    exit: "candidate_exhausted",
                    fallback_exit: "candidate_exhausted_fallback",
                    saw_transport_failure,
                },
                last_failure,
                saw_inflight_saturation,
            );
        };
        if excluded_profiles.contains(&candidate_name) {
            continue;
        }
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_candidate={} excluded_count={}",
                candidate_name,
                excluded_profiles.len()
            ),
        );
        let session_affinity_candidate = compact_followup_profile
            .as_ref()
            .is_some_and(|(owner, _)| owner == &candidate_name)
            || session_profile.as_deref() == Some(candidate_name.as_str());
        if runtime_profile_inflight_hard_limited_for_context(
            shared,
            &candidate_name,
            "compact_http",
        )? && !session_affinity_candidate
        {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "profile_inflight_saturated",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("profile", candidate_name.as_str()),
                        runtime_proxy_log_field(
                            "hard_limit",
                            runtime_proxy_profile_inflight_hard_limit().to_string(),
                        ),
                    ],
                ),
            );
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
            continue;
        }

        match attempt_runtime_standard_request(
            request_id,
            request,
            shared,
            &candidate_name,
            runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
                route_kind: RuntimeRouteKind::Compact,
                candidate_name: &candidate_name,
                strict_affinity_profile: compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                pinned_profile: None,
                turn_state_profile: None,
                session_profile: session_profile.as_deref(),
                trusted_previous_response_affinity: false,
            }),
        )? {
            RuntimeStandardAttempt::Success {
                profile_name,
                response,
            } => {
                commit_runtime_proxy_profile_selection_with_notice(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Compact,
                )?;
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_committed profile={profile_name}"
                    ),
                );
                return Ok(response);
            }
            RuntimeStandardAttempt::StaleContinuation { response } => return Ok(response),
            RuntimeStandardAttempt::TransportFailed {
                profile_name,
                stage,
            } => {
                saw_transport_failure = true;
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_transport_failure profile={profile_name} route=compact stage={stage}"
                    ),
                );
                if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
                    route_kind: RuntimeRouteKind::Compact,
                    candidate_name: &profile_name,
                    strict_affinity_profile: compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    pinned_profile: None,
                    turn_state_profile: None,
                    session_profile: session_profile.as_deref(),
                    trusted_previous_response_affinity: false,
                }) {
                    log_runtime_proxy_compact_final_failure(
                        shared,
                        RuntimeProxyCompactFinalFailureLog {
                            request_id,
                            exit: "hard_affinity_transport_failure",
                            reason: "transport",
                            selection_attempts,
                            selection_started_at,
                            pressure_mode,
                            last_failure_kind: runtime_proxy_compact_last_failure_kind(
                                last_failure.as_ref(),
                                saw_transport_failure,
                            ),
                            saw_inflight_saturation,
                            saw_transport_failure,
                            profile_name: Some(&profile_name),
                        },
                    );
                    return Ok(build_runtime_proxy_json_error_response(
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    ));
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeStandardAttempt::RetryableFailure {
                profile_name,
                response,
                overload,
            } => {
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
                    continue;
                }
                let mut released_affinity = false;
                let mut released_compact_lineage = false;
                if !overload {
                    released_affinity = release_runtime_quota_blocked_affinity(
                        shared,
                        &profile_name,
                        None,
                        None,
                        request_session_id.as_deref(),
                    )?;
                    released_compact_lineage = release_runtime_compact_lineage(
                        shared,
                        &profile_name,
                        request_session_id.as_deref(),
                        None,
                        "quota_blocked",
                    )?;
                    if session_profile.as_deref() == Some(profile_name.as_str()) {
                        session_profile = None;
                    }
                    if compact_followup_profile
                        .as_ref()
                        .is_some_and(|(owner, _)| owner == &profile_name)
                    {
                        compact_followup_profile = None;
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
                    last_failure = Some((response, false));
                    continue;
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
                    && runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
                        route_kind: RuntimeRouteKind::Compact,
                        candidate_name: &profile_name,
                        strict_affinity_profile: compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        pinned_profile: None,
                        turn_state_profile: None,
                        session_profile: session_profile.as_deref(),
                        trusted_previous_response_affinity: false,
                    })
                {
                    log_runtime_proxy_compact_final_failure(
                        shared,
                        RuntimeProxyCompactFinalFailureLog {
                            request_id,
                            exit: "hard_affinity_retryable_failure",
                            reason: "quota",
                            selection_attempts,
                            selection_started_at,
                            pressure_mode,
                            last_failure_kind: runtime_proxy_compact_last_failure_kind(
                                last_failure.as_ref(),
                                saw_transport_failure,
                            ),
                            saw_inflight_saturation,
                            saw_transport_failure,
                            profile_name: Some(&profile_name),
                        },
                    );
                    return Ok(response);
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
                    log_runtime_proxy_compact_final_failure(
                        shared,
                        RuntimeProxyCompactFinalFailureLog {
                            request_id,
                            exit: "quota_fallback_exhausted",
                            reason: "quota",
                            selection_attempts,
                            selection_started_at,
                            pressure_mode,
                            last_failure_kind: runtime_proxy_compact_last_failure_kind(
                                last_failure.as_ref(),
                                saw_transport_failure,
                            ),
                            saw_inflight_saturation,
                            saw_transport_failure,
                            profile_name: Some(&profile_name),
                        },
                    );
                    return Ok(response);
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((response, !overload));
            }
            RuntimeStandardAttempt::AuthFailed {
                profile_name,
                response,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_auth_failed profile={profile_name}"
                    ),
                );
                if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
                    route_kind: RuntimeRouteKind::Compact,
                    candidate_name: &profile_name,
                    strict_affinity_profile: compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    pinned_profile: None,
                    turn_state_profile: None,
                    session_profile: session_profile.as_deref(),
                    trusted_previous_response_affinity: false,
                }) {
                    log_runtime_proxy_compact_final_failure(
                        shared,
                        RuntimeProxyCompactFinalFailureLog {
                            request_id,
                            exit: "hard_affinity_auth_failure",
                            reason: "auth",
                            selection_attempts,
                            selection_started_at,
                            pressure_mode,
                            last_failure_kind: runtime_proxy_compact_last_failure_kind(
                                last_failure.as_ref(),
                                saw_transport_failure,
                            ),
                            saw_inflight_saturation,
                            saw_transport_failure,
                            profile_name: Some(&profile_name),
                        },
                    );
                    return Ok(response);
                }
                let released_affinity = release_runtime_auth_failed_affinity(
                    shared,
                    &profile_name,
                    None,
                    None,
                    request_session_id.as_deref(),
                )?;
                let released_compact_lineage = release_runtime_compact_lineage(
                    shared,
                    &profile_name,
                    request_session_id.as_deref(),
                    request_turn_state.as_deref(),
                    "auth_failed",
                )?;
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if compact_followup_profile
                    .as_ref()
                    .is_some_and(|(owner, _)| owner == &profile_name)
                {
                    compact_followup_profile = None;
                }
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http auth_failed_affinity_released profile={profile_name} route=compact"
                        ),
                    );
                }
                if released_compact_lineage {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http compact_lineage_released profile={profile_name} reason=auth_failed"
                        ),
                    );
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((response, true));
            }
            RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http local_selection_blocked profile={profile_name} route=compact reason=quota_exhausted_before_send"
                    ),
                );
                excluded_profiles.insert(profile_name);
            }
        }
    }
}
