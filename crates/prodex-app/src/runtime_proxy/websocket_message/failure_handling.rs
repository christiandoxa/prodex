use super::*;
use runtime_proxy_crate::runtime_proxy_websocket_error_payload_text;

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
            RuntimeWebsocketAttempt::Rejected {
                profile_name,
                payload,
            } => self.handle_upstream_rejected(profile_name, payload),
            RuntimeWebsocketAttempt::PreviousResponseNotFound {
                profile_name,
                payload,
                turn_state,
                invalid_previous_response_id,
            } => {
                if invalid_previous_response_id {
                    return self.handle_invalid_previous_response_id(profile_name, payload);
                }
                let action = self.handle_previous_response_not_found(
                    &profile_name,
                    turn_state,
                    Some("direct_current_profile_fallback"),
                    reason.previous_response_not_found_policy(),
                    false,
                )?;
                self.apply_previous_response_not_found_action(action, payload)
            }
            RuntimeWebsocketAttempt::TransportFailed {
                profile_name,
                stage,
            } => self.handle_direct_current_transport_failed(profile_name, stage),
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
            RuntimeWebsocketAttempt::Rejected {
                profile_name,
                payload,
            } => self.handle_upstream_rejected(profile_name, payload),
            RuntimeWebsocketAttempt::LocalSelectionBlocked {
                profile_name,
                reason,
            } => self.handle_candidate_local_selection_blocked(profile_name, reason),
            RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name,
                event,
            } => self.handle_reuse_watchdog_tripped(profile_name, event, turn_state_override),
            RuntimeWebsocketAttempt::TransportFailed {
                profile_name,
                stage,
            } => self.handle_candidate_transport_failed(profile_name, stage),
            RuntimeWebsocketAttempt::PreviousResponseNotFound {
                profile_name,
                payload,
                turn_state,
                invalid_previous_response_id,
            } => {
                if invalid_previous_response_id {
                    return self.handle_invalid_previous_response_id(profile_name, payload);
                }
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

    fn handle_invalid_previous_response_id(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        let owner_matches = self.bound_profile.as_deref() == Some(profile_name.as_str());
        let owner_transport_generation =
            self.previous_response_id
                .as_deref()
                .and_then(|response_id| {
                    self.websocket_session
                        .response_transport_generation(response_id)
                });
        let transport_generation = self.websocket_session.transport_generation();
        let crossed_transport_generation = owner_transport_generation
            .is_some_and(|owner_generation| owner_generation != transport_generation);
        let recovery_signal = self.previous_response_id.is_some()
            && self.request_session_id.is_some()
            && owner_matches;
        if let Some(previous_response_id) = self.previous_response_id.as_deref() {
            clear_runtime_dead_response_bindings(
                self.shared,
                &profile_name,
                &[previous_response_id.to_string()],
                "invalid_previous_response_id",
            )?;
        }
        if recovery_signal {
            let _ = commit_runtime_proxy_profile_selection_with_policy(
                self.shared,
                &profile_name,
                RuntimeRouteKind::Websocket,
                false,
            )?;
        }
        let (payload, action) = if recovery_signal {
            (
                runtime_proxy_crate::runtime_translate_invalid_previous_response_websocket_error(
                    payload,
                ),
                "codex_full_context_retry_signal",
            )
        } else {
            (payload, "pass_through")
        };
        let (logical_provider, transport_provider_hash) =
            runtime_response_trace_provider_labels(self.shared, &profile_name);
        runtime_proxy_log(
            self.shared,
            runtime_proxy_structured_log_message(
                "invalid_previous_response_id",
                [
                    runtime_proxy_log_field("request", self.request_id.to_string()),
                    runtime_proxy_log_field("websocket_session", self.session_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field(
                        "owner_transport_generation",
                        owner_transport_generation
                            .map(|generation| generation.to_string())
                            .unwrap_or_else(|| "unknown".to_string()),
                    ),
                    runtime_proxy_log_field(
                        "transport_generation",
                        transport_generation.to_string(),
                    ),
                    runtime_proxy_log_field(
                        "previous_response_id_hash",
                        runtime_proxy_crate::runtime_proxy_identifier_hash(
                            self.previous_response_id.as_deref(),
                        ),
                    ),
                    runtime_proxy_log_field(
                        "owner_profile_hash",
                        runtime_proxy_crate::runtime_proxy_identifier_hash(
                            self.bound_profile.as_deref(),
                        ),
                    ),
                    runtime_proxy_log_field(
                        "failing_profile_hash",
                        runtime_proxy_crate::runtime_proxy_identifier_hash(Some(&profile_name)),
                    ),
                    runtime_proxy_log_field("logical_provider", logical_provider),
                    runtime_proxy_log_field("transport_provider_hash", transport_provider_hash),
                    runtime_proxy_log_field(
                        "session_id_hash",
                        runtime_proxy_crate::runtime_proxy_identifier_hash(
                            self.request_session_id.as_deref(),
                        ),
                    ),
                    runtime_proxy_log_field(
                        "thread_id_hash",
                        runtime_proxy_crate::runtime_proxy_identifier_hash(
                            runtime_proxy_crate::runtime_request_thread_id(&self.handshake_request)
                                .as_deref(),
                        ),
                    ),
                    runtime_proxy_log_field(
                        "turn_id_hash",
                        runtime_proxy_crate::runtime_proxy_identifier_hash(
                            runtime_proxy_crate::runtime_request_turn_id(&self.handshake_request)
                                .as_deref(),
                        ),
                    ),
                    runtime_proxy_log_field("retry_attempt", "0"),
                    runtime_proxy_log_field("rotation_generation", "0"),
                    runtime_proxy_log_field(
                        "compaction_generation",
                        runtime_proxy_crate::runtime_request_compaction_generation(
                            &self.handshake_request,
                        )
                        .map(|generation| generation.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    ),
                    runtime_proxy_log_field("full_context_request", "false"),
                    runtime_proxy_log_field("stream_committed", "false"),
                    runtime_proxy_log_field(
                        "chain_reuse_reason",
                        if crossed_transport_generation {
                            "upstream_websocket_reconnect"
                        } else if owner_matches {
                            "bound_profile_affinity"
                        } else {
                            "unbound_previous_response"
                        },
                    ),
                    runtime_proxy_log_field("action", action),
                    runtime_proxy_log_field("compatibility_gate", recovery_signal.to_string()),
                ],
            ),
        );
        self.handle_upstream_rejected(profile_name, payload)
    }

    fn handle_upstream_rejected(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} upstream_rejected profile_hash={} action=pass_through",
                self.request_id,
                self.session_id,
                runtime_proxy_crate::runtime_proxy_identifier_hash(Some(&profile_name)),
            ),
        );
        forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
        Ok(RuntimeWebsocketMessageLoopAction::Finished)
    }

    pub(super) fn handle_direct_current_transport_failed(
        &mut self,
        profile_name: String,
        stage: &'static str,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        self.handle_transport_failed(profile_name, stage, Some("direct_current_profile_fallback"))
    }

    pub(super) fn handle_candidate_transport_failed(
        &mut self,
        profile_name: String,
        stage: &'static str,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        self.handle_transport_failed(profile_name, stage, None)
    }

    fn handle_transport_failed(
        &mut self,
        profile_name: String,
        stage: &'static str,
        via: Option<&'static str>,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        let via_suffix = via.map(|via| format!(" via={via}")).unwrap_or_default();
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} transport_failed profile={} stage={}{}",
                self.request_id, self.session_id, profile_name, stage, via_suffix
            ),
        );
        self.saw_transport_failure = true;
        self.excluded_profiles.insert(profile_name);
        self.last_failure = Some((
            RuntimeUpstreamFailureResponse::Websocket(RuntimeWebsocketErrorPayload::Text(
                runtime_proxy_websocket_error_payload_text(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                ),
            )),
            true,
        ));
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
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
            if self.try_signal_quota_full_context_retry(&profile_name)? {
                return Ok(RuntimeWebsocketMessageLoopAction::Finished);
            }
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
        if !self.prepare_quota_fallback(&profile_name)? {
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
        if reason != "profile_inflight_saturated" {
            mark_runtime_profile_retry_backoff(self.shared, &profile_name)?;
        }
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
        let request_model_name =
            runtime_smart_context_model_name_from_body(self.request_text.as_bytes());
        mark_runtime_profile_quota_quarantine_for_request_model(
            self.shared,
            &profile_name,
            RuntimeRouteKind::Websocket,
            quota_message.as_deref(),
            request_model_name.as_deref(),
        )?;
        if !self.quota_blocked_affinity_is_releasable(
            &profile_name,
            self.request_requires_previous_response_affinity,
        ) {
            if self.try_signal_quota_full_context_retry(&profile_name)? {
                return Ok(RuntimeWebsocketMessageLoopAction::Finished);
            }
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
        if !self.prepare_quota_fallback(&profile_name)? {
            forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
            return Ok(RuntimeWebsocketMessageLoopAction::Finished);
        }
        self.excluded_profiles.insert(profile_name);
        self.last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), true));
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }

    fn try_signal_quota_full_context_retry(&mut self, profile_name: &str) -> Result<bool> {
        if self.previous_response_id.is_none()
            || self.request_session_id.is_none()
            || self.bound_profile.as_deref() != Some(profile_name)
            || !self.prepare_quota_fallback(profile_name)?
        {
            return Ok(false);
        }

        let released_affinity = self.release_quota_blocked_affinity(profile_name)?;
        self.clear_profile_affinity(profile_name, true);
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} quota_blocked_full_context_retry_signal profile={} affinity_released={released_affinity}",
                self.request_id, self.session_id, profile_name
            ),
        );
        send_runtime_proxy_websocket_error(
            &mut *self.local_socket,
            400,
            "previous_response_not_found",
            "Previous response was not found. Retrying the full request.",
        )?;
        Ok(true)
    }

    fn prepare_quota_fallback(&mut self, profile_name: &str) -> Result<bool> {
        let mut excluded_profiles = self.excluded_profiles.clone();
        excluded_profiles.insert(profile_name.to_string());
        if runtime_has_route_eligible_quota_fallback(
            self.shared,
            profile_name,
            &excluded_profiles,
            RuntimeRouteKind::Websocket,
        )? {
            return Ok(true);
        }
        if self.previous_response_id.is_some()
            || self.request_requires_previous_response_affinity
            || self.request_turn_state.is_some()
            || self.pinned_profile.is_some()
            || self.turn_state_profile.is_some()
            || self.compact_followup_profile.is_some()
        {
            return Ok(false);
        }
        let Some(fallback_profile) = runtime_quota_last_chance_profile_for_route(
            self.shared,
            &excluded_profiles,
            RuntimeRouteKind::Websocket,
            self.prompt_cache_key.as_deref(),
        )?
        else {
            return Ok(false);
        };
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} quota_last_chance profile={} failed_profile={}",
                self.request_id, self.session_id, fallback_profile, profile_name
            ),
        );
        self.quota_last_chance_profile = Some(fallback_profile);
        Ok(true)
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
        self.saw_overload_failure = true;
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
        if reason != "profile_inflight_saturated" {
            mark_runtime_profile_retry_backoff(self.shared, &profile_name)?;
        }
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

#[cfg(test)]
#[path = "../../../tests/src/runtime_proxy/websocket_message/failure_handling.rs"]
mod tests;
