use super::*;

impl<'a> RuntimeWebsocketTextMessageFlow<'a> {
    pub(super) fn handle_reuse_watchdog_tripped(
        &mut self,
        profile_name: String,
        event: &'static str,
        turn_state_override: Option<&str>,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        let reuse_terminal_idle = self.websocket_session.last_terminal_elapsed();
        let retry_same_profile_with_fresh_connect =
            self.reuse_watchdog_owner_retry_needed(&profile_name);
        let nonreplayable_previous_response_reuse =
            runtime_websocket_previous_response_reuse_is_nonreplayable(
                self.previous_response_id.as_deref(),
                false,
                turn_state_override,
            );
        let stale_previous_response_reuse = runtime_websocket_previous_response_reuse_is_stale(
            nonreplayable_previous_response_reuse,
            reuse_terminal_idle,
            self.shared
                .runtime_config
                .tuning
                .websocket_previous_response_reuse_stale_ms,
        );
        runtime_proxy_log(
            self.shared,
            runtime_proxy_structured_log_message(
                "websocket_reuse_watchdog_timeout",
                [
                    runtime_proxy_log_field("request", self.request_id.to_string()),
                    runtime_proxy_log_field("websocket_session", self.session_id.to_string()),
                    runtime_proxy_log_field("profile", profile_name.as_str()),
                    runtime_proxy_log_field("event", event),
                ],
            ),
        );
        if self.reuse_watchdog_connection_limit_retry(&profile_name, event) {
            return Ok(RuntimeWebsocketMessageLoopAction::Continue);
        }
        if nonreplayable_previous_response_reuse && self.request_requires_previous_response_affinity
        {
            return self.handle_locked_affinity_watchdog(&profile_name, event);
        }
        if nonreplayable_previous_response_reuse {
            return self.handle_nonreplayable_watchdog(
                &profile_name,
                event,
                reuse_terminal_idle,
                stale_previous_response_reuse,
            );
        }
        if retry_same_profile_with_fresh_connect {
            self.schedule_websocket_reuse_fresh_retry(&profile_name);
            runtime_proxy_log(
                self.shared,
                runtime_proxy_structured_log_message(
                    "websocket_reuse_owner_fresh_retry",
                    [
                        runtime_proxy_log_field("request", self.request_id.to_string()),
                        runtime_proxy_log_field("websocket_session", self.session_id.to_string()),
                        runtime_proxy_log_field("profile", profile_name.as_str()),
                        runtime_proxy_log_field("event", event),
                    ],
                ),
            );
            return Ok(RuntimeWebsocketMessageLoopAction::Continue);
        }
        self.clear_profile_affinity(&profile_name, true);
        self.excluded_profiles.insert(profile_name);
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }

    fn reuse_watchdog_owner_retry_needed(&self, profile_name: &str) -> bool {
        !self
            .websocket_reuse_fresh_retry_profiles
            .contains(profile_name)
            && (self.bound_profile.as_deref() == Some(profile_name)
                || self.turn_state_profile.as_deref() == Some(profile_name)
                || self
                    .compact_followup_profile
                    .as_ref()
                    .is_some_and(|(owner, _)| owner == profile_name)
                || (self.request_session_id.is_some()
                    && self.session_profile.as_deref() == Some(profile_name)))
    }

    fn reuse_watchdog_connection_limit_retry(&mut self, profile_name: &str, event: &str) -> bool {
        if event != "connection_limit_reached"
            || self
                .websocket_reuse_fresh_retry_profiles
                .contains(profile_name)
        {
            return false;
        }
        self.schedule_websocket_reuse_fresh_retry(profile_name);
        runtime_proxy_log(
            self.shared,
            runtime_proxy_structured_log_message(
                "websocket_connection_limit_fresh_retry",
                [
                    runtime_proxy_log_field("request", self.request_id.to_string()),
                    runtime_proxy_log_field("websocket_session", self.session_id.to_string()),
                    runtime_proxy_log_field("profile", profile_name),
                ],
            ),
        );
        true
    }

    fn handle_locked_affinity_watchdog(
        &mut self,
        profile_name: &str,
        event: &str,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        if !self
            .websocket_reuse_fresh_retry_profiles
            .contains(profile_name)
        {
            self.schedule_websocket_reuse_fresh_retry(profile_name);
            runtime_proxy_log(
                self.shared,
                runtime_proxy_structured_log_message(
                    "websocket_reuse_locked_affinity_owner_fresh_retry",
                    [
                        runtime_proxy_log_field("request", self.request_id.to_string()),
                        runtime_proxy_log_field("websocket_session", self.session_id.to_string()),
                        runtime_proxy_log_field("profile", profile_name),
                        runtime_proxy_log_field("event", event),
                    ],
                ),
            );
            runtime_proxy_log_chain_retried_owner(
                self.shared,
                RuntimeProxyChainLog {
                    request_id: self.request_id,
                    transport: "websocket",
                    route: "websocket",
                    websocket_session: Some(self.session_id),
                    profile_name,
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
            runtime_proxy_structured_log_message(
                "stale_continuation",
                [
                    runtime_proxy_log_field("request", self.request_id.to_string()),
                    runtime_proxy_log_field("websocket_session", self.session_id.to_string()),
                    runtime_proxy_log_field("reason", "websocket_reuse_watchdog_locked_affinity"),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("event", event),
                ],
            ),
        );
        runtime_proxy_log_chain_dead_upstream_confirmed(
            self.shared,
            RuntimeProxyChainLog {
                request_id: self.request_id,
                transport: "websocket",
                route: "websocket",
                websocket_session: Some(self.session_id),
                profile_name,
                previous_response_id: self.previous_response_id.as_deref(),
                reason: "websocket_reuse_watchdog_locked_affinity",
                via: None,
            },
            Some(event),
        );
        send_runtime_proxy_stale_continuation_websocket_error(&mut *self.local_socket)?;
        Ok(RuntimeWebsocketMessageLoopAction::Finished)
    }

    fn handle_nonreplayable_watchdog(
        &mut self,
        profile_name: &str,
        event: &str,
        reuse_terminal_idle: Option<std::time::Duration>,
        stale_previous_response_reuse: bool,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        if !self
            .websocket_reuse_fresh_retry_profiles
            .contains(profile_name)
        {
            self.schedule_websocket_reuse_fresh_retry(profile_name);
            runtime_proxy_log(
                self.shared,
                runtime_proxy_structured_log_message(
                    "websocket_reuse_nonreplayable_fresh_retry",
                    [
                        runtime_proxy_log_field("request", self.request_id.to_string()),
                        runtime_proxy_log_field("websocket_session", self.session_id.to_string()),
                        runtime_proxy_log_field("profile", profile_name),
                        runtime_proxy_log_field("event", event),
                    ],
                ),
            );
            return Ok(RuntimeWebsocketMessageLoopAction::Continue);
        }
        let event_name = if stale_previous_response_reuse {
            "websocket_reuse_stale_previous_response_blocked"
        } else {
            "websocket_reuse_previous_response_blocked"
        };
        let mut fields = vec![
            runtime_proxy_log_field("request", self.request_id.to_string()),
            runtime_proxy_log_field("websocket_session", self.session_id.to_string()),
            runtime_proxy_log_field("profile", profile_name),
            runtime_proxy_log_field("event", event),
            runtime_proxy_log_field(
                "elapsed_ms",
                reuse_terminal_idle
                    .map_or(0, |elapsed| elapsed.as_millis())
                    .to_string(),
            ),
        ];
        if !stale_previous_response_reuse {
            fields.push(runtime_proxy_log_field("reason", "missing_turn_state"));
        } else {
            fields.push(runtime_proxy_log_field(
                "threshold_ms",
                self.shared
                    .runtime_config
                    .tuning
                    .websocket_previous_response_reuse_stale_ms
                    .to_string(),
            ));
        }
        runtime_proxy_log(
            self.shared,
            runtime_proxy_structured_log_message(event_name, fields),
        );
        Err(anyhow::anyhow!(
            "runtime websocket upstream closed before response.completed for previous_response_id continuation without replayable turn_state: profile={profile_name} event={event}"
        ))
    }

    fn schedule_websocket_reuse_fresh_retry(&mut self, profile_name: &str) {
        self.websocket_reuse_fresh_retry_profiles
            .insert(profile_name.to_string());
        self.websocket_reuse_fresh_retry_pending = true;
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
#[path = "../../../tests/src/runtime_proxy/websocket_message/continuation_handling.rs"]
mod tests;
