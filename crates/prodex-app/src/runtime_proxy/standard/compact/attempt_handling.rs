use super::*;

pub(super) fn handle_runtime_compact_attempt(
    context: RuntimeCompactAttemptContext<'_>,
    attempt: RuntimeStandardAttempt,
) -> Result<Option<tiny_http::ResponseBox>> {
    let RuntimeCompactAttemptContext {
        request_id,
        shared,
        candidate_has_hard_affinity,
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
        saw_overload_failure,
        saw_rate_limit_failure,
    } = context;
    match attempt {
        RuntimeStandardAttempt::Success {
            profile_name,
            response,
        } => Ok(Some(commit_runtime_proxy_compact_success(
            request_id,
            shared,
            profile_name,
            response,
        )?)),
        RuntimeStandardAttempt::StaleContinuation { response } => Ok(Some(response)),
        RuntimeStandardAttempt::RateLimited {
            profile_name,
            response,
            retry_after,
        } => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_rate_limited profile={profile_name} retry_after_ms={}",
                    retry_after.map_or(0, |delay| delay.as_millis()),
                ),
            );
            mark_runtime_profile_retry_backoff_for_delay(shared, &profile_name, retry_after)?;
            if candidate_has_hard_affinity {
                match recover_runtime_compact_hard_affinity(
                    RuntimeCompactHardAffinityRecoveryRequest {
                        request_id,
                        shared,
                        profile_name: &profile_name,
                        hard_affinity: true,
                        previous_response_profile,
                        previous_response_id: request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        request_model_name,
                        compact_followup_profile,
                        session_profile,
                        excluded_profiles,
                        reason: "compact_rate_limit",
                    },
                )? {
                    RuntimeCompactHardAffinityRecovery::Unchanged => return Ok(Some(response)),
                    RuntimeCompactHardAffinityRecovery::Retry => {}
                    RuntimeCompactHardAffinityRecovery::Return(retry) => return Ok(Some(retry)),
                }
            }
            *saw_rate_limit_failure = true;
            excluded_profiles.insert(profile_name);
            *last_failure = Some((response, RuntimeCompactFailureKind::RateLimited));
            Ok(None)
        }
        RuntimeStandardAttempt::TransportFailed {
            profile_name,
            stage,
        } => {
            *saw_transport_failure = true;
            if candidate_has_hard_affinity {
                match recover_runtime_compact_hard_affinity(
                    RuntimeCompactHardAffinityRecoveryRequest {
                        request_id,
                        shared,
                        profile_name: &profile_name,
                        hard_affinity: true,
                        previous_response_profile,
                        previous_response_id: request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        request_model_name,
                        compact_followup_profile,
                        session_profile,
                        excluded_profiles,
                        reason: "compact_transport",
                    },
                )? {
                    RuntimeCompactHardAffinityRecovery::Unchanged => {}
                    RuntimeCompactHardAffinityRecovery::Retry => {
                        excluded_profiles.insert(profile_name);
                        return Ok(None);
                    }
                    RuntimeCompactHardAffinityRecovery::Return(retry) => return Ok(Some(retry)),
                }
            }
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
                    saw_inflight_saturation: *saw_inflight_saturation,
                    saw_transport_failure: *saw_transport_failure,
                },
            ) {
                RuntimeCompactFailureFlow::Retry => {
                    excluded_profiles.insert(profile_name);
                    Ok(None)
                }
                RuntimeCompactFailureFlow::Return(response) => Ok(Some(response)),
            }
        }
        RuntimeStandardAttempt::RetryableFailure {
            profile_name,
            response,
            overload,
        } => {
            if overload {
                *saw_overload_failure = true;
            }
            match handle_runtime_proxy_compact_retryable_failure(
                RuntimeProxyCompactRetryableFailure {
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
                    saw_inflight_saturation: *saw_inflight_saturation,
                    saw_transport_failure: *saw_transport_failure,
                },
            )? {
                RuntimeCompactFailureFlow::Retry => Ok(None),
                RuntimeCompactFailureFlow::Return(response) => Ok(Some(response)),
            }
        }
        RuntimeStandardAttempt::ProfileUnavailable {
            profile_name,
            response,
        } => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_profile_unavailable profile={profile_name}"
                ),
            );
            mark_runtime_profile_retry_backoff(shared, &profile_name)?;
            if candidate_has_hard_affinity {
                match recover_runtime_compact_hard_affinity(
                    RuntimeCompactHardAffinityRecoveryRequest {
                        request_id,
                        shared,
                        profile_name: &profile_name,
                        hard_affinity: true,
                        previous_response_profile,
                        previous_response_id: request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        request_model_name,
                        compact_followup_profile,
                        session_profile,
                        excluded_profiles,
                        reason: "compact_profile_unavailable",
                    },
                )? {
                    RuntimeCompactHardAffinityRecovery::Unchanged => return Ok(Some(response)),
                    RuntimeCompactHardAffinityRecovery::Retry => {}
                    RuntimeCompactHardAffinityRecovery::Return(retry) => return Ok(Some(retry)),
                }
            }
            excluded_profiles.insert(profile_name);
            *last_failure = Some((response, RuntimeCompactFailureKind::ProfileUnavailable));
            *saw_transport_failure = true;
            Ok(None)
        }
        RuntimeStandardAttempt::AuthFailed {
            profile_name,
            response,
        } => {
            let effective_hard_affinity = if candidate_has_hard_affinity {
                match recover_runtime_compact_hard_affinity(
                    RuntimeCompactHardAffinityRecoveryRequest {
                        request_id,
                        shared,
                        profile_name: &profile_name,
                        hard_affinity: true,
                        previous_response_profile,
                        previous_response_id: request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        request_model_name,
                        compact_followup_profile,
                        session_profile,
                        excluded_profiles,
                        reason: "compact_auth_failure",
                    },
                )? {
                    RuntimeCompactHardAffinityRecovery::Unchanged => true,
                    RuntimeCompactHardAffinityRecovery::Retry => false,
                    RuntimeCompactHardAffinityRecovery::Return(retry) => return Ok(Some(retry)),
                }
            } else {
                false
            };
            match handle_runtime_proxy_compact_auth_failure(RuntimeProxyCompactAuthFailure {
                request_id,
                shared,
                profile_name,
                response,
                hard_affinity: effective_hard_affinity,
                request_session_id,
                request_turn_state,
                compact_followup_profile,
                session_profile,
                excluded_profiles,
                last_failure,
                selection_attempts,
                selection_started_at,
                pressure_mode,
                saw_inflight_saturation: *saw_inflight_saturation,
                saw_transport_failure: *saw_transport_failure,
            })? {
                RuntimeCompactFailureFlow::Retry => Ok(None),
                RuntimeCompactFailureFlow::Return(response) => Ok(Some(response)),
            }
        }
        RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
            log_runtime_compact_local_selection_blocked(request_id, shared, &profile_name);
            if candidate_has_hard_affinity
                && let RuntimeCompactHardAffinityRecovery::Return(retry) =
                    recover_runtime_compact_hard_affinity(
                        RuntimeCompactHardAffinityRecoveryRequest {
                            request_id,
                            shared,
                            profile_name: &profile_name,
                            hard_affinity: true,
                            previous_response_profile,
                            previous_response_id: request_previous_response_id,
                            request_session_id,
                            request_turn_state,
                            request_model_name,
                            compact_followup_profile,
                            session_profile,
                            excluded_profiles,
                            reason: "compact_local_selection",
                        },
                    )?
            {
                return Ok(Some(retry));
            }
            excluded_profiles.insert(profile_name);
            Ok(None)
        }
        RuntimeStandardAttempt::ProfileInflightSaturated { profile_name } => {
            log_runtime_compact_inflight_saturated(request_id, shared, &profile_name);
            *saw_inflight_saturation = true;
            Ok(None)
        }
    }
}
