use super::*;

impl<'a> RuntimeWebsocketTextMessageFlow<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        session_id: u64,
        request_id: u64,
        local_socket: &'a mut RuntimeLocalWebSocket,
        handshake_request: &RuntimeProxyRequest,
        request_text: &str,
        request_metadata: &RuntimeWebsocketRequestMetadata,
        shared: &'a RuntimeRotationProxyShared,
        websocket_session: &'a mut RuntimeWebsocketSessionState,
    ) -> Result<Self> {
        let handshake_request = handshake_request.clone();
        let request_text = request_text.to_string();
        let request_requires_previous_response_affinity =
            request_metadata.requires_previous_response_affinity;
        let previous_response_id = request_metadata.previous_response_id.clone();
        let mut request_turn_state = runtime_request_turn_state(&handshake_request);
        let explicit_request_session_id = runtime_request_explicit_session_id(&handshake_request);
        let request_session_id_header_present = explicit_request_session_id.is_some();
        let previous_response_fresh_fallback_shape =
            runtime_previous_response_fresh_fallback_shape_with_session(
                request_metadata.previous_response_fresh_fallback_shape,
                request_session_id_header_present,
            );
        let request_session_id = explicit_request_session_id
            .as_ref()
            .map(|session_id| session_id.as_str().to_string())
            .or_else(|| request_metadata.session_id.clone());
        let bound_profile = previous_response_id
            .as_deref()
            .map(|response_id| {
                runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Websocket)
            })
            .transpose()?
            .flatten();
        let trusted_previous_response_affinity = runtime_previous_response_affinity_is_trusted(
            shared,
            previous_response_id.as_deref(),
            bound_profile.as_deref(),
        )?;
        if request_turn_state.is_none()
            && let Some(turn_state) = runtime_previous_response_turn_state(
                shared,
                previous_response_id.as_deref(),
                bound_profile.as_deref(),
            )?
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket route=responses previous_response_turn_state_rehydrated response_id={} profile={} turn_state={turn_state}",
                    previous_response_id.as_deref().unwrap_or("-"),
                    bound_profile.as_deref().unwrap_or("-"),
                ),
            );
            request_turn_state = Some(turn_state);
        }
        let turn_state_profile = request_turn_state
            .as_deref()
            .map(|value| runtime_turn_state_bound_profile(shared, value))
            .transpose()?
            .flatten();
        let mut flow = Self {
            session_id,
            request_id,
            local_socket,
            handshake_request,
            request_text,
            shared,
            websocket_session,
            request_requires_previous_response_affinity,
            previous_response_id,
            request_turn_state,
            explicit_request_session_id,
            request_session_id_header_present,
            previous_response_fresh_fallback_shape,
            request_session_id,
            bound_profile,
            trusted_previous_response_affinity,
            turn_state_profile,
            bound_session_profile: None,
            compact_followup_profile: None,
            compact_session_profile: None,
            session_profile: None,
            pinned_profile: None,
            excluded_profiles: BTreeSet::new(),
            last_failure: None,
            previous_response_retry_candidate: None,
            previous_response_retry_index: 0,
            candidate_turn_state_retry_profile: None,
            candidate_turn_state_retry_value: None,
            saw_inflight_saturation: false,
            saw_previous_response_not_found: false,
            websocket_reuse_fresh_retry_profiles: BTreeSet::new(),
        };
        flow.recompute_route_affinity("initial")?;
        Ok(flow)
    }

    pub(super) fn recompute_route_affinity(&mut self, reason: &'static str) -> Result<()> {
        refresh_and_log_runtime_response_route_affinity(
            self.shared,
            self.request_id,
            Some(self.session_id),
            reason,
            self.previous_response_id.as_deref(),
            self.bound_profile.as_deref(),
            self.turn_state_profile.as_deref(),
            self.request_turn_state.as_deref(),
            self.request_session_id.as_deref(),
            self.explicit_request_session_id.as_ref(),
            self.websocket_session.profile_name.as_deref(),
            &mut self.bound_session_profile,
            &mut self.compact_followup_profile,
            &mut self.compact_session_profile,
            &mut self.session_profile,
            &mut self.pinned_profile,
        )
    }

    pub(super) fn has_continuation_priority(&self) -> bool {
        runtime_proxy_has_continuation_priority(
            self.previous_response_id.as_deref(),
            self.pinned_profile.as_deref(),
            self.request_turn_state.as_deref(),
            self.turn_state_profile.as_deref(),
            runtime_noncompact_session_priority_profile(
                self.session_profile.as_deref(),
                self.compact_session_profile.as_deref(),
            ),
        )
    }

    pub(super) fn select_candidate(&self) -> Result<Option<String>> {
        select_runtime_response_candidate_for_route_with_selection(
            self.shared,
            RuntimeResponseCandidateSelection {
                excluded_profiles: &self.excluded_profiles,
                strict_affinity_profile: self
                    .compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                pinned_profile: self.pinned_profile.as_deref(),
                turn_state_profile: self.turn_state_profile.as_deref(),
                session_profile: self.session_profile.as_deref(),
                discover_previous_response_owner: self.previous_response_id.is_some(),
                previous_response_id: self.previous_response_id.as_deref(),
                route_kind: RuntimeRouteKind::Websocket,
            },
        )
    }

    pub(super) fn turn_state_override_for(&self, candidate_name: &str) -> Option<String> {
        if self.candidate_turn_state_retry_profile.as_deref() == Some(candidate_name) {
            self.candidate_turn_state_retry_value.clone()
        } else {
            self.request_turn_state.clone()
        }
    }

    pub(super) fn log_candidate(&self, candidate_name: &str, turn_state_override: Option<&str>) {
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                self.request_id,
                self.session_id,
                candidate_name,
                self.pinned_profile,
                self.turn_state_profile,
                turn_state_override,
                self.excluded_profiles.len()
            ),
        );
    }

    pub(super) fn candidate_inflight_saturated(&mut self, candidate_name: &str) -> Result<bool> {
        let session_affinity_candidate = self.session_profile.as_deref() == Some(candidate_name);
        if self.previous_response_id.is_none()
            && self.pinned_profile.is_none()
            && self.turn_state_profile.is_none()
            && !session_affinity_candidate
            && runtime_profile_inflight_hard_limited_for_context(
                self.shared,
                candidate_name,
                "websocket_session",
            )?
        {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} profile_inflight_saturated profile={} hard_limit={}",
                    self.request_id,
                    self.session_id,
                    candidate_name,
                    runtime_proxy_profile_inflight_hard_limit(),
                ),
            );
            self.excluded_profiles.insert(candidate_name.to_string());
            self.saw_inflight_saturation = true;
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn attempt_profile(
        &mut self,
        profile_name: &str,
        turn_state_override: Option<&str>,
    ) -> Result<RuntimeWebsocketAttempt> {
        let promote_committed_profile = self.should_promote_committed_profile();
        attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
            request_id: self.request_id,
            local_socket: &mut *self.local_socket,
            handshake_request: &self.handshake_request,
            request_text: &self.request_text,
            request_previous_response_id: self.previous_response_id.as_deref(),
            request_session_id: self.request_session_id.as_deref(),
            request_turn_state: self.request_turn_state.as_deref(),
            shared: self.shared,
            websocket_session: &mut *self.websocket_session,
            profile_name,
            turn_state_override,
            promote_committed_profile,
        })
    }

    pub(super) fn should_promote_committed_profile(&self) -> bool {
        runtime_websocket_should_promote_committed_profile(
            self.previous_response_id.as_deref(),
            self.bound_profile.as_deref(),
            self.request_turn_state.as_deref(),
            self.turn_state_profile.as_deref(),
            self.compact_followup_profile.as_ref(),
            self.request_session_id_header_present,
            self.bound_session_profile.as_deref(),
        )
    }

    pub(super) fn request_requires_locked_previous_response_affinity(&self) -> bool {
        runtime_websocket_request_requires_locked_previous_response_affinity(
            self.request_requires_previous_response_affinity,
            self.trusted_previous_response_affinity,
            self.previous_response_id.as_deref(),
            self.request_turn_state.as_deref(),
        )
    }

    pub(super) fn quota_blocked_affinity_is_releasable(
        &self,
        profile_name: &str,
        request_requires_previous_response_affinity: bool,
    ) -> bool {
        runtime_quota_blocked_affinity_is_releasable(
            RuntimeCandidateAffinity {
                route_kind: RuntimeRouteKind::Websocket,
                candidate_name: profile_name,
                strict_affinity_profile: self
                    .compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                pinned_profile: self.pinned_profile.as_deref(),
                turn_state_profile: self.turn_state_profile.as_deref(),
                session_profile: self.session_profile.as_deref(),
                trusted_previous_response_affinity: self.trusted_previous_response_affinity,
            },
            request_requires_previous_response_affinity,
            self.previous_response_fresh_fallback_shape,
        )
    }

    pub(super) fn release_quota_blocked_affinity(&self, profile_name: &str) -> Result<bool> {
        release_runtime_quota_blocked_affinity(
            self.shared,
            profile_name,
            self.previous_response_id.as_deref(),
            self.request_turn_state.as_deref(),
            self.request_session_id.as_deref(),
        )
    }

    pub(super) fn clear_profile_affinity(
        &mut self,
        profile_name: &str,
        reset_previous_response_retry_index: bool,
    ) {
        clear_runtime_response_profile_affinity(
            profile_name,
            &mut self.bound_profile,
            &mut self.session_profile,
            &mut self.candidate_turn_state_retry_profile,
            &mut self.candidate_turn_state_retry_value,
            &mut self.pinned_profile,
            &mut self.previous_response_retry_index,
            reset_previous_response_retry_index,
            &mut self.turn_state_profile,
            None,
        );
    }

    pub(super) fn mark_overload_backoff(&self, profile_name: &str) -> Result<()> {
        mark_runtime_profile_retry_backoff(self.shared, profile_name)?;
        let _ = bump_runtime_profile_health_score(
            self.shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
            "websocket_overload",
        );
        let _ = bump_runtime_profile_bad_pairing_score(
            self.shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
            "websocket_overload",
        );
        Ok(())
    }

    pub(super) fn send_final_failure(&mut self) -> Result<()> {
        send_runtime_proxy_final_websocket_failure(
            &mut *self.local_socket,
            self.last_failure.take(),
            self.saw_inflight_saturation,
        )
    }

    pub(super) fn last_failure_label(&self) -> &'static str {
        match &self.last_failure {
            Some((RuntimeUpstreamFailureResponse::Websocket(_), _)) => "websocket",
            Some((RuntimeUpstreamFailureResponse::Http(_), _)) => "http",
            None => "none",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{
        test_runtime_local_websocket_pair, test_runtime_shared, test_runtime_websocket_flow,
    };
    use super::*;

    #[test]
    fn turn_state_override_prefers_retry_value_for_matching_candidate() {
        let _guard = acquire_test_runtime_lock();
        let shared = test_runtime_shared("setup-turn-state-override");
        let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
        let mut websocket_session = RuntimeWebsocketSessionState::default();
        let mut flow =
            test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
        flow.request_turn_state = Some("turn-original".to_string());
        flow.candidate_turn_state_retry_profile = Some("alpha".to_string());
        flow.candidate_turn_state_retry_value = Some("turn-retry".to_string());

        assert_eq!(
            flow.turn_state_override_for("alpha"),
            Some("turn-retry".to_string())
        );
        assert_eq!(
            flow.turn_state_override_for("beta"),
            Some("turn-original".to_string())
        );
    }

    #[test]
    fn should_promote_committed_profile_only_for_fresh_requests() {
        let _guard = acquire_test_runtime_lock();
        let shared = test_runtime_shared("setup-promote-committed");
        let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
        let mut websocket_session = RuntimeWebsocketSessionState::default();
        let mut flow =
            test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

        assert!(flow.should_promote_committed_profile());

        flow.bound_session_profile = Some("alpha".to_string());
        assert!(!flow.should_promote_committed_profile());
    }
}
