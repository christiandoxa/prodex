use super::*;

impl<'a> RuntimeWebsocketTextMessageFlow<'a> {
    pub(super) fn full_context_retry_available(&mut self, profile_name: &str) -> Result<bool> {
        if !runtime_proxy_crate::runtime_full_context_retry_signal_eligible(
            self.previous_response_id.is_some(),
            self.request_session_id.is_some(),
            self.bound_profile.as_deref() == Some(profile_name),
        ) || !self.prepare_full_context_fallback(profile_name)?
        {
            return Ok(false);
        }
        Ok(true)
    }

    pub(super) fn try_rotate_quota_turn_state_full_context(
        &mut self,
        profile_name: &str,
        payload: RuntimeWebsocketErrorPayload,
    ) -> Result<bool> {
        let replayable = self.previous_response_id.is_none()
            && self.request_turn_state.is_some()
            && self.turn_state_profile.as_deref() == Some(profile_name)
            && self.compact_followup_profile.is_none()
            && runtime_proxy_crate::runtime_request_text_has_reconstructable_full_history(
                &self.request_text,
            );
        if !replayable || !self.prepare_full_context_fallback(profile_name)? {
            return Ok(false);
        }

        let released_affinity = self.release_quota_blocked_affinity(profile_name)?;
        self.clear_profile_affinity(profile_name, true);
        self.handshake_request =
            runtime_proxy_crate::runtime_request_without_turn_state(&self.handshake_request);
        if let Some(rewritten) =
            runtime_proxy_crate::runtime_request_text_without_turn_state(&self.request_text)
        {
            self.request_text = rewritten;
        }
        self.request_turn_state = None;
        self.excluded_profiles.insert(profile_name.to_string());
        self.last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), true));
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} quota_blocked_turn_state_full_context_replay profile={} affinity_released={released_affinity}",
                self.request_id, self.session_id, profile_name
            ),
        );
        Ok(true)
    }

    pub(super) fn send_full_context_retry_signal(
        &mut self,
        profile_name: &str,
        event: &str,
        released_affinity: bool,
    ) -> Result<()> {
        self.clear_profile_affinity(profile_name, true);
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} {event} profile={} affinity_released={released_affinity}",
                self.request_id, self.session_id, profile_name
            ),
        );
        send_runtime_proxy_websocket_error(
            &mut *self.local_socket,
            400,
            "previous_response_not_found",
            "Previous response was not found. Retrying the full request.",
        )
    }

    pub(super) fn try_signal_quota_full_context_retry(
        &mut self,
        profile_name: &str,
    ) -> Result<bool> {
        if !self.full_context_retry_available(profile_name)? {
            return Ok(false);
        }
        let released_affinity = self.release_quota_blocked_affinity(profile_name)?;
        self.send_full_context_retry_signal(
            profile_name,
            "quota_blocked_full_context_retry_signal",
            released_affinity,
        )?;
        Ok(true)
    }

    pub(super) fn prepare_quota_fallback(&mut self, profile_name: &str) -> Result<bool> {
        let has_context_constraint = self.previous_response_id.is_some()
            || self.request_requires_previous_response_affinity
            || self.request_turn_state.is_some()
            || self.pinned_profile.is_some()
            || self.turn_state_profile.is_some()
            || self.compact_followup_profile.is_some();
        self.prepare_quota_fallback_with_context(profile_name, has_context_constraint)
    }

    fn prepare_full_context_fallback(&mut self, profile_name: &str) -> Result<bool> {
        if crate::runtime_has_route_recoverable_quota_fallback_for_model(
            self.shared,
            profile_name,
            &self.excluded_profiles,
            RuntimeRouteKind::Websocket,
            runtime_smart_context_model_name_from_body(self.request_text.as_bytes()).as_deref(),
        )? {
            return Ok(true);
        }
        self.prepare_quota_fallback_with_context(profile_name, false)
    }

    fn prepare_quota_fallback_with_context(
        &mut self,
        profile_name: &str,
        has_context_constraint: bool,
    ) -> Result<bool> {
        let mut excluded_profiles = self.excluded_profiles.clone();
        excluded_profiles.insert(profile_name.to_string());
        let route_eligible_fallback = runtime_has_route_eligible_quota_fallback_for_model(
            self.shared,
            profile_name,
            &excluded_profiles,
            RuntimeRouteKind::Websocket,
            runtime_smart_context_model_name_from_body(self.request_text.as_bytes()).as_deref(),
        )?;
        match runtime_proxy_crate::runtime_websocket_quota_fallback_plan(
            route_eligible_fallback,
            has_context_constraint,
        ) {
            runtime_proxy_crate::RuntimeWebsocketQuotaFallbackPlan::Ready => Ok(true),
            runtime_proxy_crate::RuntimeWebsocketQuotaFallbackPlan::Unavailable => Ok(false),
            runtime_proxy_crate::RuntimeWebsocketQuotaFallbackPlan::LastChance => {
                let Some(fallback_profile) = runtime_quota_last_chance_profile_for_route(
                    self.shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Websocket,
                    self.prompt_cache_key.as_deref(),
                    runtime_smart_context_model_name_from_body(self.request_text.as_bytes())
                        .as_deref(),
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
        }
    }
}
