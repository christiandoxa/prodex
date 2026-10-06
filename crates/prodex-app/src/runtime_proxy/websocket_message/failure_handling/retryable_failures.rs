use super::*;

impl<'a> RuntimeWebsocketTextMessageFlow<'a> {
    fn handle_rate_limited(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
        retry_after: Option<Duration>,
        via: &'static str,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} rate_limited profile={} via={} retry_after_ms={}",
                self.request_id,
                self.session_id,
                profile_name,
                via,
                retry_after.map_or(0, |delay| delay.as_millis()),
            ),
        );
        mark_runtime_profile_retry_backoff_for_delay(self.shared, &profile_name, retry_after)?;
        let plan = runtime_proxy_crate::runtime_websocket_failure_disposition(
            !self.candidate_has_hard_affinity(&profile_name),
            false,
        );
        if !plan.continue_selection {
            if self.full_context_retry_available(&profile_name)? {
                let released_affinity = release_runtime_retryable_failure_affinity(
                    self.shared,
                    &profile_name,
                    self.previous_response_id.as_deref(),
                    self.request_turn_state.as_deref(),
                    self.request_session_id.as_deref(),
                    "rate_limit_full_context_retry",
                )?;
                self.send_full_context_retry_signal(
                    &profile_name,
                    "rate_limit_full_context_retry_signal",
                    released_affinity,
                )?;
                return Ok(RuntimeWebsocketMessageLoopAction::Finished);
            }
            forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        self.saw_rate_limit_failure = true;
        if plan.exclude_profile {
            self.excluded_profiles.insert(profile_name);
        }
        self.last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }

    fn handle_auth_failed(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
        via: &'static str,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} auth_failed profile={} via={}",
                self.request_id, self.session_id, profile_name, via
            ),
        );
        let plan = runtime_proxy_crate::runtime_websocket_failure_disposition(
            !self.candidate_has_hard_affinity(&profile_name),
            false,
        );
        if !plan.continue_selection {
            if self.full_context_retry_available(&profile_name)? {
                let released_affinity = release_runtime_auth_failed_affinity(
                    self.shared,
                    &profile_name,
                    self.previous_response_id.as_deref(),
                    self.request_turn_state.as_deref(),
                    self.request_session_id.as_deref(),
                )?;
                self.send_full_context_retry_signal(
                    &profile_name,
                    "auth_failed_full_context_retry_signal",
                    released_affinity,
                )?;
                return Ok(RuntimeWebsocketMessageLoopAction::Finished);
            }
            forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        let _ = release_runtime_auth_failed_affinity(
            self.shared,
            &profile_name,
            self.previous_response_id.as_deref(),
            self.request_turn_state.as_deref(),
            self.request_session_id.as_deref(),
        )?;
        self.clear_profile_affinity(&profile_name, true);
        if plan.exclude_profile {
            self.excluded_profiles.insert(profile_name);
        }
        self.last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), true));
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }

    pub(super) fn handle_direct_current_rate_limited(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
        retry_after: Option<Duration>,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        self.handle_rate_limited(
            profile_name,
            payload,
            retry_after,
            "direct_current_profile_fallback",
        )
    }

    pub(super) fn handle_candidate_rate_limited(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
        retry_after: Option<Duration>,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        self.handle_rate_limited(profile_name, payload, retry_after, "candidate")
    }

    pub(super) fn handle_direct_current_auth_failed(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        self.handle_auth_failed(profile_name, payload, "direct_current_profile_fallback")
    }

    pub(super) fn handle_candidate_auth_failed(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        self.handle_auth_failed(profile_name, payload, "candidate")
    }
}
