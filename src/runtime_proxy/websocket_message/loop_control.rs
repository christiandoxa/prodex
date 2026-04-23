use super::*;

impl<'a> RuntimeWebsocketTextMessageFlow<'a> {
    pub(super) fn run(&mut self) -> Result<()> {
        let selection_started_at = Instant::now();
        let mut selection_attempts = 0usize;
        loop {
            let pressure_mode = runtime_proxy_pressure_mode_active_for_route(
                self.shared,
                RuntimeRouteKind::Websocket,
            );
            if runtime_proxy_precommit_budget_exhausted(
                selection_started_at,
                selection_attempts,
                self.has_continuation_priority(),
                pressure_mode,
            ) {
                match self.handle_precommit_budget_exhausted(
                    selection_started_at,
                    selection_attempts,
                    pressure_mode,
                )? {
                    RuntimeWebsocketMessageLoopAction::Continue => continue,
                    RuntimeWebsocketMessageLoopAction::Finished => return Ok(()),
                }
            }

            let Some(candidate_name) = self.select_candidate()? else {
                match self.handle_candidate_exhausted()? {
                    RuntimeWebsocketMessageLoopAction::Continue => continue,
                    RuntimeWebsocketMessageLoopAction::Finished => return Ok(()),
                }
            };
            selection_attempts = selection_attempts.saturating_add(1);
            let turn_state_override = self.turn_state_override_for(&candidate_name);
            self.log_candidate(&candidate_name, turn_state_override.as_deref());
            if self.candidate_inflight_saturated(&candidate_name)? {
                continue;
            }

            let attempt = self.attempt_profile(&candidate_name, turn_state_override.as_deref())?;
            match self.handle_candidate_attempt(attempt, turn_state_override.as_deref())? {
                RuntimeWebsocketMessageLoopAction::Continue => continue,
                RuntimeWebsocketMessageLoopAction::Finished => return Ok(()),
            }
        }
    }

    pub(super) fn handle_precommit_budget_exhausted(
        &mut self,
        selection_started_at: Instant,
        selection_attempts: usize,
        pressure_mode: bool,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} precommit_budget_exhausted attempts={} elapsed_ms={} pressure_mode={}",
                self.request_id,
                self.session_id,
                selection_attempts,
                selection_started_at.elapsed().as_millis(),
                pressure_mode,
            ),
        );
        if let Some((profile_name, source)) = self.compact_followup_profile.clone() {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} compact_fresh_fallback_blocked profile={} source={} reason=precommit_budget_exhausted",
                    self.request_id, self.session_id, profile_name, source
                ),
            );
            self.send_final_failure()?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        if let Some(action) = self.try_direct_current_profile_fallback(
            RuntimeWebsocketDirectCurrentFallbackReason::PrecommitBudgetExhausted,
        )? {
            return Ok(action);
        }
        self.send_final_failure()?;
        Ok(RuntimeWebsocketMessageLoopAction::Finished)
    }

    pub(super) fn handle_candidate_exhausted(
        &mut self,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} candidate_exhausted last_failure={}",
                self.request_id,
                self.session_id,
                self.last_failure_label(),
            ),
        );
        if let Some((profile_name, source)) = self.compact_followup_profile.clone() {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} compact_fresh_fallback_blocked profile={} source={} reason=candidate_exhausted",
                    self.request_id, self.session_id, profile_name, source
                ),
            );
            self.send_final_failure()?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        let remaining_cold_start_profiles =
            runtime_remaining_sync_probe_cold_start_profiles_for_route(
                self.shared,
                &self.excluded_profiles,
                RuntimeRouteKind::Websocket,
            )?;
        if remaining_cold_start_profiles > 0 {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} candidate_exhausted_continue route=websocket remaining_cold_start_profiles={}",
                    self.request_id, self.session_id, remaining_cold_start_profiles
                ),
            );
            runtime_proxy_sync_probe_pressure_pause(self.shared, RuntimeRouteKind::Websocket);
            return Ok(RuntimeWebsocketMessageLoopAction::Continue);
        }
        if let Some(action) = self.try_direct_current_profile_fallback(
            RuntimeWebsocketDirectCurrentFallbackReason::CandidateExhausted,
        )? {
            return Ok(action);
        }
        self.send_final_failure()?;
        Ok(RuntimeWebsocketMessageLoopAction::Finished)
    }

    pub(super) fn try_direct_current_profile_fallback(
        &mut self,
        reason: RuntimeWebsocketDirectCurrentFallbackReason,
    ) -> Result<Option<RuntimeWebsocketMessageLoopAction>> {
        if !self.allows_direct_current_profile_fallback() {
            return Ok(None);
        }
        let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
            self.shared,
            &self.excluded_profiles,
            RuntimeRouteKind::Websocket,
        )?
        else {
            return Ok(None);
        };
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} direct_current_profile_fallback profile={} reason={}",
                self.request_id,
                self.session_id,
                current_profile,
                reason.as_str(),
            ),
        );
        let turn_state_override = self.request_turn_state.clone();
        let attempt = self.attempt_profile(&current_profile, turn_state_override.as_deref())?;
        self.handle_direct_current_fallback_attempt(reason, attempt)
            .map(Some)
    }

    pub(super) fn allows_direct_current_profile_fallback(&self) -> bool {
        runtime_proxy_allows_direct_current_profile_fallback(
            self.previous_response_id.as_deref(),
            self.pinned_profile.as_deref(),
            self.request_turn_state.as_deref(),
            self.turn_state_profile.as_deref(),
            runtime_noncompact_session_priority_profile(
                self.session_profile.as_deref(),
                self.compact_session_profile.as_deref(),
            ),
            self.saw_inflight_saturation,
            self.last_failure.is_some(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{
        test_runtime_local_websocket_pair, test_runtime_shared, test_runtime_websocket_flow,
    };
    use super::*;

    #[test]
    fn direct_current_profile_fallback_requires_fresh_request_context() {
        let _guard = acquire_test_runtime_lock();
        let cases = [
            (Some("resp-1"), None, None, None, None, false, false, false),
            (None, Some("alpha"), None, None, None, false, false, false),
            (None, None, Some("ts-1"), None, None, false, false, false),
            (None, None, None, Some("alpha"), None, false, false, false),
            (None, None, None, None, Some("alpha"), false, false, false),
            (None, None, None, None, None, true, false, false),
            (None, None, None, None, None, false, true, false),
            (None, None, None, None, None, false, false, true),
        ];

        for (
            previous_response_id,
            pinned_profile,
            request_turn_state,
            turn_state_profile,
            session_profile,
            saw_inflight_saturation,
            saw_failure,
            expected,
        ) in cases
        {
            let shared = test_runtime_shared("loop-direct-current");
            let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
            let mut websocket_session = RuntimeWebsocketSessionState::default();
            let mut flow =
                test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
            flow.previous_response_id = previous_response_id.map(str::to_string);
            flow.pinned_profile = pinned_profile.map(str::to_string);
            flow.request_turn_state = request_turn_state.map(str::to_string);
            flow.turn_state_profile = turn_state_profile.map(str::to_string);
            flow.session_profile = session_profile.map(str::to_string);
            flow.saw_inflight_saturation = saw_inflight_saturation;
            if saw_failure {
                flow.last_failure = Some((
                    RuntimeUpstreamFailureResponse::Websocket(RuntimeWebsocketErrorPayload::Empty),
                    false,
                ));
            }

            assert_eq!(flow.allows_direct_current_profile_fallback(), expected);
        }
    }
}
