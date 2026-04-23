use super::*;

fn runtime_websocket_should_promote_committed_profile(
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    compact_followup_profile: Option<&(String, &'static str)>,
    request_session_id_header_present: bool,
    bound_session_profile: Option<&str>,
) -> bool {
    previous_response_id.is_none()
        && bound_profile.is_none()
        && request_turn_state.is_none()
        && turn_state_profile.is_none()
        && compact_followup_profile.is_none()
        && !(request_session_id_header_present || bound_session_profile.is_some())
}

#[cfg(test)]
#[allow(dead_code)]
pub(super) fn runtime_profile_auth_summary_for_selection(
    profile_name: &str,
    codex_home: &Path,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
) -> AuthSummary {
    runtime_profile_auth_summary_for_selection_with_policy(
        profile_name,
        codex_home,
        profile_usage_auth,
        profile_probe_cache,
        true,
    )
    .unwrap_or_else(runtime_profile_uncached_auth_summary_for_selection)
}

pub(super) fn runtime_profile_uncached_auth_summary_for_selection() -> AuthSummary {
    AuthSummary {
        label: "uncached-auth".to_string(),
        quota_compatible: false,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn runtime_profile_auth_summary_for_selection_with_policy(
    profile_name: &str,
    codex_home: &Path,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    allow_disk_fallback: bool,
) -> Option<AuthSummary> {
    runtime_profile_cached_auth_summary_from_maps_for_selection(
        profile_name,
        profile_usage_auth,
        profile_probe_cache,
    )
    .or_else(|| allow_disk_fallback.then(|| read_auth_summary(codex_home)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn proxy_runtime_websocket_text_message(
    session_id: u64,
    request_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    request_text: &str,
    request_metadata: &RuntimeWebsocketRequestMetadata,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
) -> Result<()> {
    RuntimeWebsocketTextMessageFlow::new(
        session_id,
        request_id,
        local_socket,
        handshake_request,
        request_text,
        request_metadata,
        shared,
        websocket_session,
    )?
    .run()
}

struct RuntimeWebsocketTextMessageFlow<'a> {
    session_id: u64,
    request_id: u64,
    local_socket: &'a mut RuntimeLocalWebSocket,
    handshake_request: RuntimeProxyRequest,
    request_text: String,
    shared: &'a RuntimeRotationProxyShared,
    websocket_session: &'a mut RuntimeWebsocketSessionState,
    request_requires_previous_response_affinity: bool,
    previous_response_id: Option<String>,
    request_turn_state: Option<String>,
    explicit_request_session_id: Option<RuntimeExplicitSessionId>,
    request_session_id_header_present: bool,
    previous_response_fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    request_session_id: Option<String>,
    bound_profile: Option<String>,
    trusted_previous_response_affinity: bool,
    turn_state_profile: Option<String>,
    bound_session_profile: Option<String>,
    compact_followup_profile: Option<(String, &'static str)>,
    compact_session_profile: Option<String>,
    session_profile: Option<String>,
    pinned_profile: Option<String>,
    excluded_profiles: BTreeSet<String>,
    last_failure: Option<(RuntimeUpstreamFailureResponse, bool)>,
    previous_response_retry_candidate: Option<String>,
    previous_response_retry_index: usize,
    candidate_turn_state_retry_profile: Option<String>,
    candidate_turn_state_retry_value: Option<String>,
    saw_inflight_saturation: bool,
    saw_previous_response_not_found: bool,
    websocket_reuse_fresh_retry_profiles: BTreeSet<String>,
}

#[derive(Clone, Copy)]
enum RuntimeWebsocketMessageLoopAction {
    Continue,
    Finished,
}

#[derive(Clone, Copy)]
enum RuntimeWebsocketDirectCurrentFallbackReason {
    PrecommitBudgetExhausted,
    CandidateExhausted,
}

impl RuntimeWebsocketDirectCurrentFallbackReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::PrecommitBudgetExhausted => "precommit_budget_exhausted",
            Self::CandidateExhausted => "candidate_exhausted",
        }
    }

    fn previous_response_not_found_policy(self) -> RuntimePreviousResponseNotFoundPolicy {
        RuntimePreviousResponseNotFoundPolicy::websocket(
            matches!(self, Self::PrecommitBudgetExhausted),
            false,
        )
    }

    fn reset_previous_response_retry_index_on_local_block(self) -> bool {
        matches!(self, Self::PrecommitBudgetExhausted)
    }
}

impl<'a> RuntimeWebsocketTextMessageFlow<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
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

    fn run(&mut self) -> Result<()> {
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

    fn recompute_route_affinity(&mut self, reason: &'static str) -> Result<()> {
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

    fn has_continuation_priority(&self) -> bool {
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

    fn select_candidate(&self) -> Result<Option<String>> {
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

    fn turn_state_override_for(&self, candidate_name: &str) -> Option<String> {
        if self.candidate_turn_state_retry_profile.as_deref() == Some(candidate_name) {
            self.candidate_turn_state_retry_value.clone()
        } else {
            self.request_turn_state.clone()
        }
    }

    fn log_candidate(&self, candidate_name: &str, turn_state_override: Option<&str>) {
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

    fn candidate_inflight_saturated(&mut self, candidate_name: &str) -> Result<bool> {
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

    fn handle_precommit_budget_exhausted(
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

    fn handle_candidate_exhausted(&mut self) -> Result<RuntimeWebsocketMessageLoopAction> {
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

    fn try_direct_current_profile_fallback(
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

    fn allows_direct_current_profile_fallback(&self) -> bool {
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

    fn attempt_profile(
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

    fn should_promote_committed_profile(&self) -> bool {
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

    fn handle_direct_current_fallback_attempt(
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

    fn handle_candidate_attempt(
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

    fn handle_direct_current_quota_blocked(
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

    fn handle_direct_current_overloaded(
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

    fn handle_direct_current_local_selection_blocked(
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

    fn handle_candidate_quota_blocked(
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

    fn handle_candidate_overloaded(
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

    fn handle_candidate_local_selection_blocked(
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

    fn handle_reuse_watchdog_tripped(
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

    fn handle_previous_response_not_found(
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

    fn apply_previous_response_not_found_action(
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

    fn request_requires_locked_previous_response_affinity(&self) -> bool {
        runtime_websocket_request_requires_locked_previous_response_affinity(
            self.request_requires_previous_response_affinity,
            self.trusted_previous_response_affinity,
            self.previous_response_id.as_deref(),
            self.request_turn_state.as_deref(),
        )
    }

    fn quota_blocked_affinity_is_releasable(
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

    fn release_quota_blocked_affinity(&self, profile_name: &str) -> Result<bool> {
        release_runtime_quota_blocked_affinity(
            self.shared,
            profile_name,
            self.previous_response_id.as_deref(),
            self.request_turn_state.as_deref(),
            self.request_session_id.as_deref(),
        )
    }

    fn clear_profile_affinity(
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

    fn mark_overload_backoff(&self, profile_name: &str) -> Result<()> {
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

    fn send_final_failure(&mut self) -> Result<()> {
        send_runtime_proxy_final_websocket_failure(
            &mut *self.local_socket,
            self.last_failure.take(),
            self.saw_inflight_saturation,
        )
    }

    fn last_failure_label(&self) -> &'static str {
        match &self.last_failure {
            Some((RuntimeUpstreamFailureResponse::Websocket(_), _)) => "websocket",
            Some((RuntimeUpstreamFailureResponse::Http(_), _)) => "http",
            None => "none",
        }
    }
}
