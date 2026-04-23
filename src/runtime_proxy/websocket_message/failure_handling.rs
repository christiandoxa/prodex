use super::*;

impl<'a> RuntimeWebsocketTextMessageFlow<'a> {
    pub(super) fn handle_direct_current_fallback_attempt(
        &mut self,
        reason: RuntimeWebsocketDirectCurrentFallbackReason,
        attempt: RuntimeWebsocketAttempt,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        match attempt {
            RuntimeWebsocketAttempt::Delivered => Ok(RuntimeWebsocketMessageLoopAction::Finished),
            RuntimeWebsocketAttempt::QuotaBlocked {
                profile_name,
                payload,
            } => self.handle_direct_current_quota_blocked(profile_name, payload),
            RuntimeWebsocketAttempt::Overloaded {
                profile_name,
                payload,
            } => self.handle_direct_current_overloaded(profile_name, payload),
            RuntimeWebsocketAttempt::PreviousResponseNotFound {
                profile_name,
                payload,
                turn_state,
            } => {
                let action = self.handle_previous_response_not_found(
                    &profile_name,
                    turn_state,
                    Some("direct_current_profile_fallback"),
                    reason.previous_response_not_found_policy(),
                    false,
                )?;
                self.apply_previous_response_not_found_action(action, payload)
            }
            RuntimeWebsocketAttempt::ReuseWatchdogTripped { profile_name, .. } => {
                self.excluded_profiles.insert(profile_name);
                Ok(RuntimeWebsocketMessageLoopAction::Continue)
            }
            RuntimeWebsocketAttempt::LocalSelectionBlocked {
                profile_name,
                reason: block_reason,
            } => self.handle_direct_current_local_selection_blocked(
                profile_name,
                block_reason,
                reason.reset_previous_response_retry_index_on_local_block(),
            ),
        }
    }

    pub(super) fn handle_candidate_attempt(
        &mut self,
        attempt: RuntimeWebsocketAttempt,
        turn_state_override: Option<&str>,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        match attempt {
            RuntimeWebsocketAttempt::Delivered => Ok(RuntimeWebsocketMessageLoopAction::Finished),
            RuntimeWebsocketAttempt::QuotaBlocked {
                profile_name,
                payload,
            } => self.handle_candidate_quota_blocked(profile_name, payload),
            RuntimeWebsocketAttempt::Overloaded {
                profile_name,
                payload,
            } => self.handle_candidate_overloaded(profile_name, payload),
            RuntimeWebsocketAttempt::LocalSelectionBlocked {
                profile_name,
                reason,
            } => self.handle_candidate_local_selection_blocked(profile_name, reason),
            RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name,
                event,
            } => self.handle_reuse_watchdog_tripped(profile_name, event, turn_state_override),
            RuntimeWebsocketAttempt::PreviousResponseNotFound {
                profile_name,
                payload,
                turn_state,
            } => {
                let action = self.handle_previous_response_not_found(
                    &profile_name,
                    turn_state,
                    None,
                    RuntimePreviousResponseNotFoundPolicy::websocket(false, true),
                    true,
                )?;
                self.apply_previous_response_not_found_action(action, payload)
            }
        }
    }

    pub(super) fn handle_direct_current_quota_blocked(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        mark_runtime_profile_retry_backoff(self.shared, &profile_name)?;
        if !self.quota_blocked_affinity_is_releasable(
            &profile_name,
            self.request_requires_previous_response_affinity,
        ) {
            forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        let released_affinity = self.release_quota_blocked_affinity(&profile_name)?;
        self.clear_profile_affinity(&profile_name, true);
        if released_affinity {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} quota_blocked_affinity_released profile={} via=direct_current_profile_fallback",
                    self.request_id, self.session_id, profile_name
                ),
            );
        }
        if !runtime_has_route_eligible_quota_fallback(
            self.shared,
            &profile_name,
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
        )? {
            forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        self.excluded_profiles.insert(profile_name);
        self.last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), true));
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }

    pub(super) fn handle_direct_current_overloaded(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        let overload_message =
            extract_runtime_proxy_overload_message_from_websocket_payload(&payload);
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} upstream_overloaded route=websocket profile={} via=direct_current_profile_fallback message={}",
                self.request_id,
                self.session_id,
                profile_name,
                overload_message.as_deref().unwrap_or("-"),
            ),
        );
        self.mark_overload_backoff(&profile_name)?;
        if !self.quota_blocked_affinity_is_releasable(
            &profile_name,
            self.request_requires_locked_previous_response_affinity(),
        ) {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} upstream_overload_passthrough route=websocket profile={} reason=hard_affinity via=direct_current_profile_fallback",
                    self.request_id, self.session_id, profile_name
                ),
            );
            forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        self.excluded_profiles.insert(profile_name);
        self.last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }

    pub(super) fn handle_direct_current_local_selection_blocked(
        &mut self,
        profile_name: String,
        reason: &'static str,
        reset_previous_response_retry_index: bool,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        mark_runtime_profile_retry_backoff(self.shared, &profile_name)?;
        if !self.quota_blocked_affinity_is_releasable(
            &profile_name,
            self.request_requires_locked_previous_response_affinity(),
        ) {
            send_runtime_proxy_websocket_error(
                &mut *self.local_socket,
                503,
                "service_unavailable",
                runtime_proxy_local_selection_failure_message(),
            )?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        let released_affinity = self.release_quota_blocked_affinity(&profile_name)?;
        self.clear_profile_affinity(&profile_name, reset_previous_response_retry_index);
        if released_affinity {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} quota_blocked_affinity_released profile={} reason={} via=direct_current_profile_fallback",
                    self.request_id, self.session_id, profile_name, reason
                ),
            );
        }
        self.excluded_profiles.insert(profile_name);
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }

    pub(super) fn handle_candidate_quota_blocked(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        let quota_message = extract_runtime_proxy_quota_message_from_websocket_payload(&payload);
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} quota_blocked profile={}",
                self.request_id, self.session_id, profile_name
            ),
        );
        mark_runtime_profile_quota_quarantine(
            self.shared,
            &profile_name,
            RuntimeRouteKind::Websocket,
            quota_message.as_deref(),
        )?;
        if !self.quota_blocked_affinity_is_releasable(
            &profile_name,
            self.request_requires_previous_response_affinity,
        ) {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} upstream_usage_limit_passthrough route=websocket profile={} reason=hard_affinity",
                    self.request_id, self.session_id, profile_name
                ),
            );
            forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        let released_affinity = self.release_quota_blocked_affinity(&profile_name)?;
        self.clear_profile_affinity(&profile_name, true);
        if released_affinity {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} quota_blocked_affinity_released profile={}",
                    self.request_id, self.session_id, profile_name
                ),
            );
        }
        if !runtime_has_route_eligible_quota_fallback(
            self.shared,
            &profile_name,
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
        )? {
            forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        self.excluded_profiles.insert(profile_name);
        self.last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), true));
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }

    pub(super) fn handle_candidate_overloaded(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        let overload_message =
            extract_runtime_proxy_overload_message_from_websocket_payload(&payload);
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} upstream_overloaded route=websocket profile={} message={}",
                self.request_id,
                self.session_id,
                profile_name,
                overload_message.as_deref().unwrap_or("-"),
            ),
        );
        self.mark_overload_backoff(&profile_name)?;
        if !self.quota_blocked_affinity_is_releasable(
            &profile_name,
            self.request_requires_locked_previous_response_affinity(),
        ) {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} upstream_overload_passthrough route=websocket profile={} reason=hard_affinity",
                    self.request_id, self.session_id, profile_name
                ),
            );
            forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        self.excluded_profiles.insert(profile_name);
        self.last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }

    pub(super) fn handle_candidate_local_selection_blocked(
        &mut self,
        profile_name: String,
        reason: &'static str,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} local_selection_blocked profile={} reason={}",
                self.request_id, self.session_id, profile_name, reason
            ),
        );
        mark_runtime_profile_retry_backoff(self.shared, &profile_name)?;
        if !self.quota_blocked_affinity_is_releasable(
            &profile_name,
            self.request_requires_locked_previous_response_affinity(),
        ) {
            send_runtime_proxy_websocket_error(
                &mut *self.local_socket,
                503,
                "service_unavailable",
                runtime_proxy_local_selection_failure_message(),
            )?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        let released_affinity = self.release_quota_blocked_affinity(&profile_name)?;
        self.clear_profile_affinity(&profile_name, true);
        if released_affinity {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} quota_blocked_affinity_released profile={} reason={}",
                    self.request_id, self.session_id, profile_name, reason
                ),
            );
        }
        self.excluded_profiles.insert(profile_name);
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }
}
