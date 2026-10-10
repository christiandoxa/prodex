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
        let affinity_releasable = !self.candidate_has_hard_affinity(&profile_name);
        let full_context_retry_available = if affinity_releasable {
            false
        } else {
            self.full_context_retry_available(&profile_name)?
        };
        let plan = self.websocket_failure_plan(
            runtime_proxy_crate::RuntimeWebsocketFailureClass::RateLimited,
            affinity_releasable,
            false,
            full_context_retry_available,
            true,
            via == "direct_current_profile_fallback",
        )?;
        if plan.mark_backoff {
            mark_runtime_profile_retry_backoff_for_delay(self.shared, &profile_name, retry_after)?;
        }
        match plan.action {
            runtime_proxy_crate::RuntimeWebsocketFailureAction::FullContextRetry => {
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
            runtime_proxy_crate::RuntimeWebsocketFailureAction::PassThrough => {
                forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
                return Ok(RuntimeWebsocketMessageLoopAction::Finished);
            }
            runtime_proxy_crate::RuntimeWebsocketFailureAction::Rotate
            | runtime_proxy_crate::RuntimeWebsocketFailureAction::Continue => {}
            _ => {
                return Err(anyhow::anyhow!(
                    "invalid rate-limit WebSocket failure action {:?}",
                    plan.action
                ));
            }
        }
        self.saw_rate_limit_failure |= plan.record_rate_limit_failure;
        if plan.exclude_profile {
            self.excluded_profiles.insert(profile_name);
        }
        if plan.store_last_failure {
            self.last_failure = Some((
                RuntimeUpstreamFailureResponse::Websocket(payload),
                plan.last_failure_retryable,
            ));
        }
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
        let affinity_releasable = !self.candidate_has_hard_affinity(&profile_name);
        let full_context_retry_available = if affinity_releasable {
            false
        } else {
            self.full_context_retry_available(&profile_name)?
        };
        let plan = self.websocket_failure_plan(
            runtime_proxy_crate::RuntimeWebsocketFailureClass::AuthFailed,
            affinity_releasable,
            false,
            full_context_retry_available,
            true,
            false,
        )?;
        match plan.action {
            runtime_proxy_crate::RuntimeWebsocketFailureAction::FullContextRetry => {
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
            runtime_proxy_crate::RuntimeWebsocketFailureAction::PassThrough => {
                forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
                return Ok(RuntimeWebsocketMessageLoopAction::Finished);
            }
            runtime_proxy_crate::RuntimeWebsocketFailureAction::Rotate
            | runtime_proxy_crate::RuntimeWebsocketFailureAction::Continue => {}
            _ => {
                return Err(anyhow::anyhow!(
                    "invalid auth WebSocket failure action {:?}",
                    plan.action
                ));
            }
        }
        if plan.release_affinity {
            let _ = release_runtime_auth_failed_affinity(
                self.shared,
                &profile_name,
                self.previous_response_id.as_deref(),
                self.request_turn_state.as_deref(),
                self.request_session_id.as_deref(),
            )?;
            self.clear_profile_affinity(&profile_name, true);
        }
        if plan.exclude_profile {
            self.excluded_profiles.insert(profile_name);
        }
        if plan.store_last_failure {
            self.last_failure = Some((
                RuntimeUpstreamFailureResponse::Websocket(payload),
                plan.last_failure_retryable,
            ));
        }
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

impl<'a> RuntimeWebsocketTextMessageFlow<'a> {
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
        let affinity_releasable = !self.candidate_has_hard_affinity(&profile_name);
        let full_context_retry_available = if affinity_releasable {
            false
        } else {
            self.full_context_retry_available(&profile_name)?
        };
        let plan = self.websocket_failure_plan(
            runtime_proxy_crate::RuntimeWebsocketFailureClass::TransportFailed,
            affinity_releasable,
            false,
            full_context_retry_available,
            false,
            false,
        )?;
        match plan.action {
            runtime_proxy_crate::RuntimeWebsocketFailureAction::FullContextRetry => {
                let released_affinity = release_runtime_retryable_failure_affinity(
                    self.shared,
                    &profile_name,
                    self.previous_response_id.as_deref(),
                    self.request_turn_state.as_deref(),
                    self.request_session_id.as_deref(),
                    "transport_full_context_retry",
                )?;
                self.send_full_context_retry_signal(
                    &profile_name,
                    "transport_failure_full_context_retry_signal",
                    released_affinity,
                )?;
                Ok(RuntimeWebsocketMessageLoopAction::Finished)
            }
            runtime_proxy_crate::RuntimeWebsocketFailureAction::Rotate
            | runtime_proxy_crate::RuntimeWebsocketFailureAction::Continue => {
                self.saw_transport_failure |= plan.retryable_failure;
                if plan.exclude_profile {
                    self.excluded_profiles.insert(profile_name);
                }
                if plan.store_last_failure {
                    self.last_failure = Some((
                        RuntimeUpstreamFailureResponse::Websocket(
                            RuntimeWebsocketErrorPayload::Text(
                                runtime_proxy_websocket_error_payload_text(
                                    503,
                                    "service_unavailable",
                                    runtime_proxy_local_selection_failure_message(),
                                ),
                            ),
                        ),
                        plan.last_failure_retryable,
                    ));
                }
                Ok(RuntimeWebsocketMessageLoopAction::Continue)
            }
            runtime_proxy_crate::RuntimeWebsocketFailureAction::PassThrough => {
                send_runtime_proxy_websocket_error(
                    &mut *self.local_socket,
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                )?;
                Ok(RuntimeWebsocketMessageLoopAction::Finished)
            }
            action => Err(anyhow::anyhow!(
                "invalid transport WebSocket failure action {action:?}"
            )),
        }
    }

    pub(super) fn handle_direct_current_quota_blocked(
        &mut self,
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        let affinity_releasable = self.quota_blocked_affinity_is_releasable(
            &profile_name,
            self.request_requires_previous_response_affinity,
        );
        if !affinity_releasable
            && self.try_rotate_quota_turn_state_full_context(&profile_name, payload.clone())?
        {
            return Ok(RuntimeWebsocketMessageLoopAction::Continue);
        }
        let (full_context_retry_available, quota_fallback_available) =
            self.quota_recovery_availability(&profile_name, affinity_releasable)?;
        let plan = self.websocket_failure_plan(
            runtime_proxy_crate::RuntimeWebsocketFailureClass::QuotaBlocked,
            affinity_releasable,
            false,
            full_context_retry_available,
            quota_fallback_available,
            true,
        )?;
        if plan.mark_backoff {
            mark_runtime_profile_retry_backoff(self.shared, &profile_name)?;
        }
        match plan.action {
            runtime_proxy_crate::RuntimeWebsocketFailureAction::FullContextRetry => {
                if self.try_signal_quota_full_context_retry(&profile_name)? {
                    return Ok(RuntimeWebsocketMessageLoopAction::Finished);
                }
                forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
                Ok(RuntimeWebsocketMessageLoopAction::Finished)
            }
            runtime_proxy_crate::RuntimeWebsocketFailureAction::PassThrough => {
                forward_runtime_proxy_websocket_error(&mut *self.local_socket, &payload)?;
                Ok(RuntimeWebsocketMessageLoopAction::Finished)
            }
            runtime_proxy_crate::RuntimeWebsocketFailureAction::Rotate
            | runtime_proxy_crate::RuntimeWebsocketFailureAction::Continue => {
                let released_affinity = self.apply_quota_affinity_release(
                    &profile_name,
                    plan.release_affinity,
                    plan.clear_affinity,
                )?;
                if released_affinity {
                    runtime_proxy_log(
                        self.shared,
                        format!(
                            "request={} websocket_session={} quota_blocked_affinity_released profile={} via=direct_current_profile_fallback",
                            self.request_id, self.session_id, profile_name
                        ),
                    );
                }
                if plan.exclude_profile {
                    self.excluded_profiles.insert(profile_name);
                }
                if plan.store_last_failure {
                    self.last_failure = Some((
                        RuntimeUpstreamFailureResponse::Websocket(payload),
                        plan.last_failure_retryable,
                    ));
                }
                Ok(RuntimeWebsocketMessageLoopAction::Continue)
            }
            action => Err(anyhow::anyhow!(
                "invalid quota WebSocket failure action {action:?}"
            )),
        }
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
        let affinity_releasable = !self.candidate_has_hard_affinity(&profile_name);
        let full_context_retry_available = if affinity_releasable {
            false
        } else {
            self.full_context_retry_available(&profile_name)?
        };
        let plan = self.websocket_failure_plan(
            runtime_proxy_crate::RuntimeWebsocketFailureClass::Overloaded,
            affinity_releasable,
            false,
            full_context_retry_available,
            true,
            true,
        )?;
        if plan.mark_backoff {
            self.mark_overload_backoff(&profile_name)?;
        }
        match plan.action {
            runtime_proxy_crate::RuntimeWebsocketFailureAction::FullContextRetry => {
                let released_affinity = release_runtime_retryable_failure_affinity(
                    self.shared,
                    &profile_name,
                    self.previous_response_id.as_deref(),
                    self.request_turn_state.as_deref(),
                    self.request_session_id.as_deref(),
                    "overload_full_context_retry",
                )?;
                self.send_full_context_retry_signal(
                    &profile_name,
                    "upstream_overload_full_context_retry_signal",
                    released_affinity,
                )?;
                return Ok(RuntimeWebsocketMessageLoopAction::Finished);
            }
            runtime_proxy_crate::RuntimeWebsocketFailureAction::PassThrough => {
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
            runtime_proxy_crate::RuntimeWebsocketFailureAction::Rotate
            | runtime_proxy_crate::RuntimeWebsocketFailureAction::Continue => {}
            action => {
                return Err(anyhow::anyhow!(
                    "invalid overload WebSocket failure action {action:?}"
                ));
            }
        }
        if plan.exclude_profile {
            self.excluded_profiles.insert(profile_name);
        }
        if plan.store_last_failure {
            self.last_failure = Some((
                RuntimeUpstreamFailureResponse::Websocket(payload),
                plan.last_failure_retryable,
            ));
        }
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }

    pub(super) fn handle_direct_current_local_selection_blocked(
        &mut self,
        profile_name: String,
        reason: &'static str,
        reset_previous_response_retry_index: bool,
    ) -> Result<RuntimeWebsocketMessageLoopAction> {
        let affinity_releasable = !self.candidate_has_hard_affinity(&profile_name);
        let inflight_saturated = reason == "profile_inflight_saturated";
        let full_context_retry_available = if affinity_releasable || inflight_saturated {
            false
        } else {
            self.full_context_retry_available(&profile_name)?
        };
        let plan = self.websocket_failure_plan(
            runtime_proxy_crate::RuntimeWebsocketFailureClass::LocalSelectionBlocked,
            affinity_releasable,
            inflight_saturated,
            full_context_retry_available,
            true,
            true,
        )?;
        if plan.mark_backoff {
            mark_runtime_profile_retry_backoff(self.shared, &profile_name)?;
        }
        match plan.action {
            runtime_proxy_crate::RuntimeWebsocketFailureAction::FullContextRetry => {
                let released_affinity = release_runtime_retryable_failure_affinity(
                    self.shared,
                    &profile_name,
                    self.previous_response_id.as_deref(),
                    self.request_turn_state.as_deref(),
                    self.request_session_id.as_deref(),
                    "local_selection_full_context_retry",
                )?;
                self.send_full_context_retry_signal(
                    &profile_name,
                    "local_selection_full_context_retry_signal",
                    released_affinity,
                )?;
                return Ok(RuntimeWebsocketMessageLoopAction::Finished);
            }
            runtime_proxy_crate::RuntimeWebsocketFailureAction::PassThrough => {
                send_runtime_proxy_websocket_error(
                    &mut *self.local_socket,
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                )?;
                return Ok(RuntimeWebsocketMessageLoopAction::Finished);
            }
            runtime_proxy_crate::RuntimeWebsocketFailureAction::Rotate
            | runtime_proxy_crate::RuntimeWebsocketFailureAction::Continue => {}
            action => {
                return Err(anyhow::anyhow!(
                    "invalid local-pressure WebSocket failure action {action:?}"
                ));
            }
        }
        let released_affinity = if plan.clear_affinity {
            let released_affinity = self.release_quota_blocked_affinity(&profile_name)?;
            self.clear_profile_affinity(&profile_name, reset_previous_response_retry_index);
            released_affinity
        } else {
            false
        };
        if released_affinity {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} websocket_session={} quota_blocked_affinity_released profile={} reason={} via=direct_current_profile_fallback",
                    self.request_id, self.session_id, profile_name, reason
                ),
            );
        }
        if plan.exclude_profile {
            self.excluded_profiles.insert(profile_name);
        }
        Ok(RuntimeWebsocketMessageLoopAction::Continue)
    }
}
