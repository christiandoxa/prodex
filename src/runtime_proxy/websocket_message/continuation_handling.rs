use super::*;

impl<'a> RuntimeWebsocketTextMessageFlow<'a> {
    pub(super) fn handle_reuse_watchdog_tripped(
        &mut self,
        profile_name: String,
        event: &'static str,
        turn_state_override: Option<&str>,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        let reuse_terminal_idle = self.websocket_session.last_terminal_elapsed();
        let retry_same_profile_with_fresh_connect = !self
            .websocket_reuse_fresh_retry_profiles
            .contains(&profile_name)
            && (self.bound_profile.as_deref() == Some(profile_name.as_str())
                || self.turn_state_profile.as_deref() == Some(profile_name.as_str())
                || self
                    .compact_followup_profile
                    .as_ref()
                    .is_some_and(|(owner, _)| owner == &profile_name)
                || (self.request_session_id.is_some()
                    && self.session_profile.as_deref() == Some(profile_name.as_str())));
        let nonreplayable_previous_response_reuse =
            runtime_websocket_previous_response_reuse_is_nonreplayable(
                self.previous_response_id.as_deref(),
                false,
                turn_state_override,
            );
        let stale_previous_response_reuse = runtime_websocket_previous_response_reuse_is_stale(
            nonreplayable_previous_response_reuse,
            reuse_terminal_idle,
        );
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} websocket_reuse_watchdog_timeout profile={} event={}",
                self.request_id, self.session_id, profile_name, event
            ),
        );
        if nonreplayable_previous_response_reuse && self.request_requires_previous_response_affinity
        {
            if !self
                .websocket_reuse_fresh_retry_profiles
                .contains(&profile_name)
            {
                self.websocket_reuse_fresh_retry_profiles
                    .insert(profile_name.clone());
                runtime_proxy_log(
                    self.shared,
                    format!(
                        "request={} websocket_session={} websocket_reuse_locked_affinity_owner_fresh_retry profile={} event={}",
                        self.request_id, self.session_id, profile_name, event
                    ),
                );
                runtime_proxy_log_chain_retried_owner(
                    self.shared,
                    RuntimeProxyChainLog {
                        request_id: self.request_id,
                        transport: "websocket",
                        route: "websocket",
                        websocket_session: Some(self.session_id),
                        profile_name: &profile_name,
                        previous_response_id: self.previous_response_id.as_deref(),
                        reason: "websocket_reuse_watchdog_locked_affinity",
                        via: None,
                    },
                    0,
                );
                return Ok(RuntimeWebsocketMessageLoopAction::Continue);
            }
            runtime_proxy_record_continuity_failure_reason(
                self.shared,
                "stale_continuation",
                "websocket_reuse_watchdog_locked_affinity",
            );
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} stale_continuation reason=websocket_reuse_watchdog_locked_affinity profile={} event={}",
                    self.request_id, self.session_id, profile_name, event
                ),
            );
            runtime_proxy_log_chain_dead_upstream_confirmed(
                self.shared,
                RuntimeProxyChainLog {
                    request_id: self.request_id,
                    transport: "websocket",
                    route: "websocket",
                    websocket_session: Some(self.session_id),
                    profile_name: &profile_name,
                    previous_response_id: self.previous_response_id.as_deref(),
                    reason: "websocket_reuse_watchdog_locked_affinity",
                    via: None,
                },
                Some(event),
            );
            send_runtime_proxy_stale_continuation_websocket_error(&mut *self.local_socket)?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        if nonreplayable_previous_response_reuse {
            if !self
                .websocket_reuse_fresh_retry_profiles
                .contains(&profile_name)
            {
                self.websocket_reuse_fresh_retry_profiles
                    .insert(profile_name.clone());
                runtime_proxy_log(
                    self.shared,
                    format!(
                        "request={} websocket_session={} websocket_reuse_nonreplayable_fresh_retry profile={} event={}",
                        self.request_id, self.session_id, profile_name, event
                    ),
                );
                return Ok(RuntimeWebsocketMessageLoopAction::Continue);
            }
            if stale_previous_response_reuse {
                runtime_proxy_log(
                    self.shared,
                    format!(
                        "request={} websocket_session={} websocket_reuse_stale_previous_response_blocked profile={} event={} elapsed_ms={} threshold_ms={}",
                        self.request_id,
                        self.session_id,
                        profile_name,
                        event,
                        reuse_terminal_idle
                            .map(|elapsed| elapsed.as_millis())
                            .unwrap_or(0),
                        runtime_proxy_websocket_previous_response_reuse_stale_ms(),
                    ),
                );
            } else {
                runtime_proxy_log(
                    self.shared,
                    format!(
                        "request={} websocket_session={} websocket_reuse_previous_response_blocked profile={} event={} reason=missing_turn_state elapsed_ms={}",
                        self.request_id,
                        self.session_id,
                        profile_name,
                        event,
                        reuse_terminal_idle
                            .map(|elapsed| elapsed.as_millis())
                            .unwrap_or(0),
                    ),
                );
            }
            return Err(anyhow::anyhow!(
                "runtime websocket upstream closed before response.completed for previous_response_id continuation without replayable turn_state: profile={profile_name} event={event}"
            ));
        }
        if retry_same_profile_with_fresh_connect {
            self.websocket_reuse_fresh_retry_profiles
                .insert(profile_name.clone());
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} websocket_reuse_owner_fresh_retry profile={} event={}",
                    self.request_id, self.session_id, profile_name, event
                ),
            );
            return Ok(RuntimeWebsocketMessageLoopAction::Continue);
        }
        self.clear_profile_affinity(&profile_name, true);
        self.excluded_profiles.insert(profile_name);
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }

    pub(super) fn handle_previous_response_not_found(
        &mut self,
        profile_name: &str,
        turn_state: Option<String>,
        via: Option<&'static str>,
        policy: RuntimePreviousResponseNotFoundPolicy,
        update_trusted_previous_response_affinity: bool,
    ) -> Result<RuntimePreviousResponseNotFoundAction> {
        let trusted_previous_response_affinity = self.trusted_previous_response_affinity;
        let trusted_previous_response_affinity_mut = update_trusted_previous_response_affinity
            .then_some(&mut self.trusted_previous_response_affinity);
        handle_runtime_previous_response_not_found(
            RuntimePreviousResponseNotFoundContext {
                shared: self.shared,
                log_context: RuntimePreviousResponseLogContext {
                    request_id: self.request_id,
                    transport: "websocket",
                    route: "websocket",
                    websocket_session: Some(self.session_id),
                    via,
                },
                route: RuntimePreviousResponseNotFoundRoute::Websocket,
                route_kind: RuntimeRouteKind::Websocket,
                profile_name,
                turn_state,
                previous_response_id: self.previous_response_id.as_deref(),
                request_turn_state: self.request_turn_state.as_deref(),
                request_session_id: self.request_session_id.as_deref(),
                request_requires_previous_response_affinity: self
                    .request_requires_previous_response_affinity,
                trusted_previous_response_affinity,
                previous_response_fresh_fallback_used: false,
                fresh_fallback_shape: self.previous_response_fresh_fallback_shape,
                policy,
            },
            RuntimePreviousResponseNotFoundState {
                saw_previous_response_not_found: &mut self.saw_previous_response_not_found,
                previous_response_retry_candidate: &mut self.previous_response_retry_candidate,
                previous_response_retry_index: &mut self.previous_response_retry_index,
                candidate_turn_state_retry_profile: &mut self.candidate_turn_state_retry_profile,
                candidate_turn_state_retry_value: &mut self.candidate_turn_state_retry_value,
                bound_profile: &mut self.bound_profile,
                session_profile: &mut self.session_profile,
                pinned_profile: &mut self.pinned_profile,
                turn_state_profile: &mut self.turn_state_profile,
                compact_followup_profile: Some(&mut self.compact_followup_profile),
                excluded_profiles: &mut self.excluded_profiles,
                trusted_previous_response_affinity: trusted_previous_response_affinity_mut,
            },
        )
    }

    pub(super) fn apply_previous_response_not_found_action(
        &mut self,
        action: RuntimePreviousResponseNotFoundAction,
        payload: RuntimeWebsocketErrorPayload,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        match action {
            RuntimePreviousResponseNotFoundAction::RetryOwner
            | RuntimePreviousResponseNotFoundAction::Rotate => {
                self.last_failure =
                    Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                Ok(RuntimeWebsocketMessageLoopAction::Continue)
            }
            RuntimePreviousResponseNotFoundAction::StaleContinuation => {
                send_runtime_proxy_stale_continuation_websocket_error(&mut *self.local_socket)?;
                Ok(RuntimeWebsocketMessageLoopAction::Finished)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{
        read_runtime_websocket_text, test_runtime_local_websocket_pair, test_runtime_shared,
        test_runtime_websocket_flow,
    };
    use super::*;

    #[test]
    fn previous_response_not_found_rotate_stashes_last_failure() {
        let _guard = acquire_test_runtime_lock();
        let shared = test_runtime_shared("continuation-rotate");
        let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
        let mut websocket_session = RuntimeWebsocketSessionState::default();
        let mut flow =
            test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

        let action = flow
            .apply_previous_response_not_found_action(
                RuntimePreviousResponseNotFoundAction::Rotate,
                RuntimeWebsocketErrorPayload::Text("upstream error".to_string()),
            )
            .expect("rotate handling should succeed");

        assert!(matches!(
            action,
            RuntimeWebsocketMessageLoopAction::Continue
        ));
        assert!(matches!(
            flow.last_failure,
            Some((RuntimeUpstreamFailureResponse::Websocket(_), false))
        ));
    }

    #[test]
    fn stale_continuation_action_sends_error_frame_and_finishes() {
        let _guard = acquire_test_runtime_lock();
        let shared = test_runtime_shared("continuation-stale");
        let (mut local_socket, mut client_socket) = test_runtime_local_websocket_pair();
        let mut websocket_session = RuntimeWebsocketSessionState::default();
        let mut flow =
            test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

        let action = flow
            .apply_previous_response_not_found_action(
                RuntimePreviousResponseNotFoundAction::StaleContinuation,
                RuntimeWebsocketErrorPayload::Empty,
            )
            .expect("stale continuation handling should succeed");
        let frame = read_runtime_websocket_text(&mut client_socket);

        assert!(matches!(
            action,
            RuntimeWebsocketMessageLoopAction::Finished
        ));
        assert!(frame.contains("\"code\":\"stale_continuation\""));
    }
}
