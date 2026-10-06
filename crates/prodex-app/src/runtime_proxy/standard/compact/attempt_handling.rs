use super::*;

pub(super) fn handle_runtime_compact_attempt(
    mut context: RuntimeCompactAttemptContext<'_>,
    attempt: RuntimeStandardAttempt,
) -> Result<Option<tiny_http::ResponseBox>> {
    match attempt {
        RuntimeStandardAttempt::Success {
            profile_name,
            response,
        } => context.handle_success(profile_name, response),
        RuntimeStandardAttempt::StaleContinuation { response } => Ok(Some(response)),
        RuntimeStandardAttempt::RateLimited {
            profile_name,
            response,
            retry_after,
        } => context.handle_rate_limited(profile_name, response, retry_after),
        RuntimeStandardAttempt::TransportFailed {
            profile_name,
            stage,
        } => context.handle_transport_failed(profile_name, stage),
        RuntimeStandardAttempt::RetryableFailure {
            profile_name,
            response,
            overload,
        } => context.handle_retryable_failure(profile_name, response, overload),
        RuntimeStandardAttempt::ProfileUnavailable {
            profile_name,
            response,
        } => context.handle_profile_unavailable(profile_name, response),
        RuntimeStandardAttempt::AuthFailed {
            profile_name,
            response,
        } => context.handle_auth_failed(profile_name, response),
        RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
            context.handle_local_selection_blocked(profile_name)
        }
        RuntimeStandardAttempt::ProfileInflightSaturated { profile_name } => {
            context.handle_inflight_saturated(profile_name)
        }
    }
}

impl RuntimeCompactAttemptContext<'_> {
    fn recover_hard_affinity(
        &mut self,
        profile_name: &str,
        reason: &'static str,
    ) -> Result<RuntimeCompactHardAffinityRecovery> {
        recover_runtime_compact_hard_affinity(RuntimeCompactHardAffinityRecoveryRequest {
            request_id: self.request_id,
            shared: self.shared,
            profile_name,
            hard_affinity: true,
            previous_response_profile: self.previous_response_profile,
            previous_response_id: self.request_previous_response_id,
            request_session_id: self.request_session_id,
            request_turn_state: self.request_turn_state,
            request_model_name: self.request_model_name,
            compact_followup_profile: &mut *self.compact_followup_profile,
            session_profile: &mut *self.session_profile,
            excluded_profiles: &*self.excluded_profiles,
            reason,
        })
    }

    fn handle_success(
        &mut self,
        profile_name: String,
        response: tiny_http::ResponseBox,
    ) -> Result<Option<tiny_http::ResponseBox>> {
        Ok(Some(commit_runtime_proxy_compact_success(
            self.request_id,
            self.shared,
            profile_name,
            response,
        )?))
    }

    fn handle_rate_limited(
        &mut self,
        profile_name: String,
        response: tiny_http::ResponseBox,
        retry_after: Option<std::time::Duration>,
    ) -> Result<Option<tiny_http::ResponseBox>> {
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} transport=http compact_rate_limited profile={} retry_after_ms={}",
                self.request_id,
                profile_name,
                retry_after.map_or(0, |delay| delay.as_millis()),
            ),
        );
        mark_runtime_profile_retry_backoff_for_delay(self.shared, &profile_name, retry_after)?;
        if self.candidate_has_hard_affinity {
            match self.recover_hard_affinity(&profile_name, "compact_rate_limit")? {
                RuntimeCompactHardAffinityRecovery::Unchanged => return Ok(Some(response)),
                RuntimeCompactHardAffinityRecovery::Retry => {}
                RuntimeCompactHardAffinityRecovery::Return(retry) => return Ok(Some(retry)),
            }
        }
        *self.saw_rate_limit_failure = true;
        self.excluded_profiles.insert(profile_name);
        *self.last_failure = Some((response, RuntimeCompactFailureKind::RateLimited));
        Ok(None)
    }

    fn handle_transport_failed(
        &mut self,
        profile_name: String,
        stage: &'static str,
    ) -> Result<Option<tiny_http::ResponseBox>> {
        *self.saw_transport_failure = true;
        if self.candidate_has_hard_affinity {
            match self.recover_hard_affinity(&profile_name, "compact_transport")? {
                RuntimeCompactHardAffinityRecovery::Unchanged => {}
                RuntimeCompactHardAffinityRecovery::Retry => {
                    self.excluded_profiles.insert(profile_name);
                    return Ok(None);
                }
                RuntimeCompactHardAffinityRecovery::Return(retry) => return Ok(Some(retry)),
            }
        }
        match finish_runtime_proxy_compact_transport_failure(RuntimeProxyCompactTransportFailure {
            request_id: self.request_id,
            shared: self.shared,
            profile_name: &profile_name,
            stage,
            hard_affinity: self.candidate_has_hard_affinity,
            selection_attempts: self.selection_attempts,
            selection_started_at: self.selection_started_at,
            pressure_mode: self.pressure_mode,
            last_failure: self.last_failure.as_ref(),
            saw_inflight_saturation: *self.saw_inflight_saturation,
            saw_transport_failure: *self.saw_transport_failure,
        }) {
            RuntimeCompactFailureFlow::Retry => {
                self.excluded_profiles.insert(profile_name);
                Ok(None)
            }
            RuntimeCompactFailureFlow::Return(response) => Ok(Some(response)),
        }
    }

    fn handle_retryable_failure(
        &mut self,
        profile_name: String,
        response: tiny_http::ResponseBox,
        overload: bool,
    ) -> Result<Option<tiny_http::ResponseBox>> {
        if overload {
            *self.saw_overload_failure = true;
        }
        match handle_runtime_proxy_compact_retryable_failure(RuntimeProxyCompactRetryableFailure {
            request_id: self.request_id,
            shared: self.shared,
            profile_name,
            response,
            overload,
            previous_response_profile: self.previous_response_profile,
            request_previous_response_id: self.request_previous_response_id,
            request_session_id: self.request_session_id,
            request_turn_state: self.request_turn_state,
            request_model_name: self.request_model_name,
            current_profile: self.current_profile,
            compact_followup_profile: &mut *self.compact_followup_profile,
            session_profile: &mut *self.session_profile,
            auto_redeemed_profiles: &mut *self.auto_redeemed_profiles,
            conservative_overload_retried_profiles: &mut *self
                .conservative_overload_retried_profiles,
            excluded_profiles: &mut *self.excluded_profiles,
            last_failure: &mut *self.last_failure,
            selection_attempts: self.selection_attempts,
            selection_started_at: self.selection_started_at,
            pressure_mode: self.pressure_mode,
            saw_inflight_saturation: *self.saw_inflight_saturation,
            saw_transport_failure: *self.saw_transport_failure,
        })? {
            RuntimeCompactFailureFlow::Retry => Ok(None),
            RuntimeCompactFailureFlow::Return(response) => Ok(Some(response)),
        }
    }

    fn handle_profile_unavailable(
        &mut self,
        profile_name: String,
        response: tiny_http::ResponseBox,
    ) -> Result<Option<tiny_http::ResponseBox>> {
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} transport=http compact_profile_unavailable profile={profile_name}",
                self.request_id
            ),
        );
        mark_runtime_profile_retry_backoff(self.shared, &profile_name)?;
        if self.candidate_has_hard_affinity {
            match self.recover_hard_affinity(&profile_name, "compact_profile_unavailable")? {
                RuntimeCompactHardAffinityRecovery::Unchanged => return Ok(Some(response)),
                RuntimeCompactHardAffinityRecovery::Retry => {}
                RuntimeCompactHardAffinityRecovery::Return(retry) => return Ok(Some(retry)),
            }
        }
        self.excluded_profiles.insert(profile_name);
        *self.last_failure = Some((response, RuntimeCompactFailureKind::ProfileUnavailable));
        *self.saw_transport_failure = true;
        Ok(None)
    }

    fn handle_auth_failed(
        &mut self,
        profile_name: String,
        response: tiny_http::ResponseBox,
    ) -> Result<Option<tiny_http::ResponseBox>> {
        let effective_hard_affinity = if self.candidate_has_hard_affinity {
            match self.recover_hard_affinity(&profile_name, "compact_auth_failure")? {
                RuntimeCompactHardAffinityRecovery::Unchanged => true,
                RuntimeCompactHardAffinityRecovery::Retry => false,
                RuntimeCompactHardAffinityRecovery::Return(retry) => return Ok(Some(retry)),
            }
        } else {
            false
        };
        match handle_runtime_proxy_compact_auth_failure(RuntimeProxyCompactAuthFailure {
            request_id: self.request_id,
            shared: self.shared,
            profile_name,
            response,
            hard_affinity: effective_hard_affinity,
            request_session_id: self.request_session_id,
            request_turn_state: self.request_turn_state,
            compact_followup_profile: &mut *self.compact_followup_profile,
            session_profile: &mut *self.session_profile,
            excluded_profiles: &mut *self.excluded_profiles,
            last_failure: &mut *self.last_failure,
            selection_attempts: self.selection_attempts,
            selection_started_at: self.selection_started_at,
            pressure_mode: self.pressure_mode,
            saw_inflight_saturation: *self.saw_inflight_saturation,
            saw_transport_failure: *self.saw_transport_failure,
        })? {
            RuntimeCompactFailureFlow::Retry => Ok(None),
            RuntimeCompactFailureFlow::Return(response) => Ok(Some(response)),
        }
    }

    fn handle_local_selection_blocked(
        &mut self,
        profile_name: String,
    ) -> Result<Option<tiny_http::ResponseBox>> {
        log_runtime_compact_local_selection_blocked(self.request_id, self.shared, &profile_name);
        if self.candidate_has_hard_affinity {
            if let RuntimeCompactHardAffinityRecovery::Return(retry) =
                self.recover_hard_affinity(&profile_name, "compact_local_selection")?
            {
                return Ok(Some(retry));
            }
        }
        self.excluded_profiles.insert(profile_name);
        Ok(None)
    }

    fn handle_inflight_saturated(
        &mut self,
        profile_name: String,
    ) -> Result<Option<tiny_http::ResponseBox>> {
        log_runtime_compact_inflight_saturated(self.request_id, self.shared, &profile_name);
        *self.saw_inflight_saturation = true;
        Ok(None)
    }
}
