use super::super::{
    RuntimeResponseCandidateSelection, commit_runtime_proxy_profile_selection_with_notice,
    release_runtime_auth_failed_affinity, release_runtime_compact_lineage,
    runtime_compact_route_followup_bound_profile,
    runtime_profile_inflight_hard_limited_for_context, runtime_proxy_current_profile,
    runtime_proxy_log, runtime_proxy_precommit_budget_exhausted_for_route,
    runtime_proxy_pressure_mode_active_for_route, runtime_proxy_should_shed_fresh_compact_request,
    runtime_proxy_sync_probe_pressure_pause,
    runtime_remaining_sync_probe_cold_start_profiles_for_route, runtime_request_session_id,
    runtime_request_turn_state, runtime_session_bound_profile,
    select_runtime_response_candidate_for_route,
};
use super::attempt_runtime_standard_request;
use crate::runtime_proxy_shared::RuntimeStandardAttempt;
use crate::runtime_state_shared::{RuntimeRotationProxyShared, RuntimeRouteKind};
use crate::shared_types::RuntimeProxyRequest;
use anyhow::Result;
use std::collections::BTreeSet;
use std::time::Instant;
mod admission;
mod affinity;
mod fallback;
mod logging;
mod retryable;
mod transport;
use admission::{
    build_runtime_fresh_compact_pressure_response, log_runtime_compact_inflight_saturated,
};
use affinity::runtime_compact_candidate_has_hard_affinity;
use fallback::{
    RuntimeProxyCompactSelectionExhausted, finish_runtime_proxy_compact_selection_exhausted,
};
use logging::{
    RuntimeProxyCompactAttemptFailureLog, log_runtime_proxy_compact_attempt_final_failure,
};
use retryable::{
    RuntimeProxyCompactRetryableFailure, handle_runtime_proxy_compact_retryable_failure,
};
use transport::{
    RuntimeProxyCompactTransportFailure, finish_runtime_proxy_compact_transport_failure,
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
        return Ok(build_runtime_fresh_compact_pressure_response(
            request_id,
            shared,
            selection_attempts,
            selection_started_at,
            pressure_mode,
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
            log_runtime_compact_inflight_saturated(request_id, shared, &candidate_name);
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
            continue;
        }

        match attempt_runtime_standard_request(
            request_id,
            request,
            shared,
            &candidate_name,
            runtime_compact_candidate_has_hard_affinity(
                &candidate_name,
                compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                session_profile.as_deref(),
            ),
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
                if let Some(response) = finish_runtime_proxy_compact_transport_failure(
                    RuntimeProxyCompactTransportFailure {
                        request_id,
                        shared,
                        profile_name: &profile_name,
                        stage,
                        strict_affinity_profile: compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        session_profile: session_profile.as_deref(),
                        selection_attempts,
                        selection_started_at,
                        pressure_mode,
                        last_failure: last_failure.as_ref(),
                        saw_inflight_saturation,
                        saw_transport_failure,
                    },
                ) {
                    return Ok(response);
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeStandardAttempt::RetryableFailure {
                profile_name,
                response,
                overload,
            } => {
                if let Some(response) = handle_runtime_proxy_compact_retryable_failure(
                    RuntimeProxyCompactRetryableFailure {
                        request_id,
                        shared,
                        profile_name,
                        response,
                        overload,
                        request_session_id: request_session_id.as_deref(),
                        current_profile: &current_profile,
                        compact_followup_profile: &mut compact_followup_profile,
                        session_profile: &mut session_profile,
                        auto_redeemed_profiles: &mut auto_redeemed_profiles,
                        conservative_overload_retried_profiles:
                            &mut conservative_overload_retried_profiles,
                        excluded_profiles: &mut excluded_profiles,
                        last_failure: &mut last_failure,
                        selection_attempts,
                        selection_started_at,
                        pressure_mode,
                        saw_inflight_saturation,
                        saw_transport_failure,
                    },
                )? {
                    return Ok(response);
                }
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
                if runtime_compact_candidate_has_hard_affinity(
                    &profile_name,
                    compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    session_profile.as_deref(),
                ) {
                    log_runtime_proxy_compact_attempt_final_failure(
                        shared,
                        RuntimeProxyCompactAttemptFailureLog {
                            request_id,
                            exit: "hard_affinity_auth_failure",
                            reason: "auth",
                            selection_attempts,
                            selection_started_at,
                            pressure_mode,
                            last_failure: last_failure.as_ref(),
                            saw_inflight_saturation,
                            saw_transport_failure,
                            profile_name: &profile_name,
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
