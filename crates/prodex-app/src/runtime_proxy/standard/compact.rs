use super::super::{
    RuntimeResponseCandidateSelection, runtime_compact_route_followup_bound_profile,
    runtime_proxy_current_profile, runtime_proxy_log,
    runtime_proxy_precommit_budget_exhausted_for_route,
    runtime_proxy_pressure_mode_active_for_route, runtime_proxy_should_shed_fresh_compact_request,
    runtime_proxy_sync_probe_pressure_pause,
    runtime_remaining_sync_probe_cold_start_profiles_for_route, runtime_request_session_id,
    runtime_request_turn_state, runtime_session_bound_profile,
    runtime_smart_context_model_name_from_body,
    select_runtime_response_candidate_for_route_with_request,
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
mod auth;
mod commit;
mod fallback;
mod flow;
mod logging;
mod retryable;
mod transport;
use admission::{
    build_runtime_fresh_compact_pressure_response, log_runtime_compact_local_selection_blocked,
    runtime_compact_candidate_inflight_saturated,
};
use affinity::runtime_compact_route_candidate_has_hard_affinity;
use auth::{RuntimeProxyCompactAuthFailure, handle_runtime_proxy_compact_auth_failure};
use commit::commit_runtime_proxy_compact_success;
use fallback::RuntimeProxyCompactSelectionExhausted;
use fallback::finish_runtime_proxy_compact_selection_exhausted;
use flow::RuntimeCompactFailureFlow;
use logging::log_runtime_proxy_compact_candidate;
use retryable::RuntimeProxyCompactRetryableFailure;
use retryable::handle_runtime_proxy_compact_retryable_failure;
use transport::RuntimeProxyCompactTransportFailure;
use transport::finish_runtime_proxy_compact_transport_failure;

pub(super) fn proxy_runtime_compact_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    let request_model_name = runtime_smart_context_model_name_from_body(&request.body);
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
    logging::log_runtime_proxy_compact_followup_owner(
        request_id,
        shared,
        compact_followup_profile.as_ref(),
    );
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
        let Some(candidate_name) = select_runtime_response_candidate_for_route_with_request(
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
            Some(request_id),
            request_model_name.as_deref(),
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
        log_runtime_proxy_compact_candidate(
            request_id,
            shared,
            &candidate_name,
            excluded_profiles.len(),
        );
        let candidate_has_hard_affinity = runtime_compact_route_candidate_has_hard_affinity(
            &candidate_name,
            &compact_followup_profile,
            session_profile.as_deref(),
        );
        if runtime_compact_candidate_inflight_saturated(
            request_id,
            shared,
            &candidate_name,
            candidate_has_hard_affinity,
        )? {
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
            continue;
        }

        match attempt_runtime_standard_request(
            request_id,
            request,
            shared,
            &candidate_name,
            candidate_has_hard_affinity,
        )? {
            RuntimeStandardAttempt::Success {
                profile_name,
                response,
            } => {
                return commit_runtime_proxy_compact_success(
                    request_id,
                    shared,
                    profile_name,
                    response,
                );
            }
            RuntimeStandardAttempt::StaleContinuation { response } => return Ok(response),
            RuntimeStandardAttempt::TransportFailed {
                profile_name,
                stage,
            } => {
                saw_transport_failure = true;
                match finish_runtime_proxy_compact_transport_failure(
                    RuntimeProxyCompactTransportFailure {
                        request_id,
                        shared,
                        profile_name: &profile_name,
                        stage,
                        hard_affinity: candidate_has_hard_affinity,
                        selection_attempts,
                        selection_started_at,
                        pressure_mode,
                        last_failure: last_failure.as_ref(),
                        saw_inflight_saturation,
                        saw_transport_failure,
                    },
                ) {
                    RuntimeCompactFailureFlow::Retry => {
                        excluded_profiles.insert(profile_name);
                    }
                    RuntimeCompactFailureFlow::Return(response) => return Ok(response),
                }
            }
            RuntimeStandardAttempt::RetryableFailure {
                profile_name,
                response,
                overload,
            } => {
                match handle_runtime_proxy_compact_retryable_failure(
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
                    RuntimeCompactFailureFlow::Retry => {}
                    RuntimeCompactFailureFlow::Return(response) => return Ok(response),
                }
            }
            RuntimeStandardAttempt::AuthFailed {
                profile_name,
                response,
            } => {
                match handle_runtime_proxy_compact_auth_failure(RuntimeProxyCompactAuthFailure {
                    request_id,
                    shared,
                    profile_name,
                    response,
                    hard_affinity: candidate_has_hard_affinity,
                    request_session_id: request_session_id.as_deref(),
                    request_turn_state: request_turn_state.as_deref(),
                    compact_followup_profile: &mut compact_followup_profile,
                    session_profile: &mut session_profile,
                    excluded_profiles: &mut excluded_profiles,
                    last_failure: &mut last_failure,
                    selection_attempts,
                    selection_started_at,
                    pressure_mode,
                    saw_inflight_saturation,
                    saw_transport_failure,
                })? {
                    RuntimeCompactFailureFlow::Retry => {}
                    RuntimeCompactFailureFlow::Return(response) => return Ok(response),
                }
            }
            RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
                log_runtime_compact_local_selection_blocked(request_id, shared, &profile_name);
                excluded_profiles.insert(profile_name);
            }
        }
    }
}
