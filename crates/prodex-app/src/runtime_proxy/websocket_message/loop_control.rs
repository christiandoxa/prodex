use super::super::{
    RuntimeInflightReliefWait, RuntimeInflightReliefWaitResult, RuntimeResponseCandidateSelection,
    RuntimeRouteKind, RuntimeSelectionTraceDirect, RuntimeWebsocketAttempt,
    await_runtime_proxy_async_task, clear_runtime_recovered_profiles,
    runtime_noncompact_session_priority_profile, runtime_profile_recovery_wait_for_route,
    runtime_proxy_allows_direct_current_profile_fallback,
    runtime_proxy_direct_current_fallback_profile, runtime_proxy_local_capacity_timeout_message,
    runtime_proxy_log, runtime_proxy_log_field,
    runtime_proxy_maybe_wait_for_interactive_inflight_relief,
    runtime_proxy_precommit_budget_exhausted_for_route,
    runtime_proxy_precommit_budget_for_profile_count, runtime_proxy_pressure_mode_active_for_route,
    runtime_proxy_probe_refresh_pause, runtime_proxy_structured_log_message,
    runtime_remaining_sync_probe_cold_start_profiles_for_route, runtime_route_kind_label,
    runtime_selection_trace_log_direct, runtime_smart_context_model_name_from_body,
    select_runtime_response_candidate_for_route_with_request, send_runtime_proxy_websocket_error,
};
use super::{
    RuntimeWebsocketDirectCurrentFallbackReason, RuntimeWebsocketMessageLoopAction,
    RuntimeWebsocketTextMessageFlow,
};
use anyhow::Result;
use std::time::{Duration, Instant};

enum RuntimeWebsocketBudgetDecision {
    Proceed,
    Continue,
    Finished,
}

#[cfg(test)]
use super::super::{
    RuntimeUpstreamFailureResponse, RuntimeWebsocketErrorPayload, RuntimeWebsocketSessionState,
};
#[cfg(test)]
use crate::acquire_test_runtime_lock;

impl<'a> RuntimeWebsocketTextMessageFlow<'a> {
    pub(super) fn run(&mut self) -> Result<()> {
        let mut selection_started_at = Instant::now();
        let mut selection_attempts = 0usize;
        loop {
            if self.local_capacity_wait_timed_out {
                return self.finish_local_capacity_timeout();
            }
            let pressure_mode = runtime_proxy_pressure_mode_active_for_route(
                self.shared,
                RuntimeRouteKind::Websocket,
            );
            match self.precommit_budget_decision(
                selection_started_at,
                selection_attempts,
                pressure_mode,
            )? {
                RuntimeWebsocketBudgetDecision::Proceed => {}
                RuntimeWebsocketBudgetDecision::Continue => continue,
                RuntimeWebsocketBudgetDecision::Finished => return Ok(()),
            }

            let Some(candidate_name) = self.select_candidate()? else {
                match self.handle_candidate_exhausted()? {
                    RuntimeWebsocketMessageLoopAction::Continue => {
                        if std::mem::take(&mut self.reset_selection_budget) {
                            selection_started_at = Instant::now();
                            selection_attempts = 0;
                        }
                        continue;
                    }
                    RuntimeWebsocketMessageLoopAction::Finished => return Ok(()),
                }
            };
            let turn_state_override = self.turn_state_override_for(&candidate_name);
            self.log_candidate(&candidate_name, turn_state_override.as_deref());
            if self.candidate_inflight_saturated(&candidate_name, selection_started_at)? {
                continue;
            }

            let attempt = self.attempt_profile(&candidate_name, turn_state_override.as_deref())?;
            if self.handle_inflight_saturation(&attempt, selection_started_at)? {
                continue;
            }
            if !matches!(
                &attempt,
                RuntimeWebsocketAttempt::LocalSelectionBlocked { .. }
            ) {
                selection_attempts = selection_attempts.saturating_add(1);
            }
            match self.handle_candidate_attempt(attempt, turn_state_override.as_deref())? {
                RuntimeWebsocketMessageLoopAction::Continue => {
                    continue;
                }
                RuntimeWebsocketMessageLoopAction::Finished => return Ok(()),
            }
        }
    }

    fn finish_local_capacity_timeout(&mut self) -> Result<()> {
        send_runtime_proxy_websocket_error(
            &mut *self.local_socket,
            503,
            "local_capacity_timeout",
            runtime_proxy_local_capacity_timeout_message(),
        )?;
        Ok(())
    }

    fn handle_inflight_saturation(
        &mut self,
        attempt: &RuntimeWebsocketAttempt,
        selection_started_at: Instant,
    ) -> Result<bool> {
        if !matches!(
            attempt,
            RuntimeWebsocketAttempt::LocalSelectionBlocked {
                reason: "profile_inflight_saturated",
                ..
            }
        ) {
            return Ok(false);
        }
        self.saw_inflight_saturation = true;
        match runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
            request_id: self.request_id,
            request: &self.handshake_request,
            shared: self.shared,
            excluded_profiles: &self.excluded_profiles,
            route_kind: RuntimeRouteKind::Websocket,
            selection_started_at,
            continuation: self.has_continuation_priority(),
            wait_affinity_owner: None,
            selected_profile: None,
        })? {
            RuntimeInflightReliefWaitResult::Relieved
            | RuntimeInflightReliefWaitResult::NotWaitable => Ok(true),
            RuntimeInflightReliefWaitResult::DeadlineExpired => {
                self.local_capacity_wait_timed_out = true;
                Ok(true)
            }
        }
    }

    fn precommit_budget_decision(
        &mut self,
        selection_started_at: Instant,
        selection_attempts: usize,
        pressure_mode: bool,
    ) -> Result<RuntimeWebsocketBudgetDecision> {
        if !self.precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            pressure_mode,
        )? {
            return Ok(RuntimeWebsocketBudgetDecision::Proceed);
        }
        Ok(
            match self.handle_precommit_budget_exhausted(
                selection_started_at,
                selection_attempts,
                pressure_mode,
            )? {
                RuntimeWebsocketMessageLoopAction::Continue => {
                    RuntimeWebsocketBudgetDecision::Continue
                }
                RuntimeWebsocketMessageLoopAction::Finished => {
                    RuntimeWebsocketBudgetDecision::Finished
                }
            },
        )
    }

    fn precommit_budget_exhausted(
        &mut self,
        selection_started_at: Instant,
        selection_attempts: usize,
        pressure_mode: bool,
    ) -> Result<bool> {
        if std::mem::take(&mut self.websocket_reuse_fresh_retry_pending) {
            return Ok(false);
        }
        let exhausted = runtime_proxy_precommit_budget_exhausted_for_route(
            self.shared,
            selection_started_at,
            selection_attempts,
            self.has_continuation_priority(),
            pressure_mode,
        )?;
        if self.recovery_sweeps == 0 {
            if self.saw_transport_failure
                && !self.has_continuation_priority()
                && selection_attempts < self.profile_count()?
            {
                return Ok(false);
            }
            return Ok(exhausted);
        }
        let profile_count = self
            .shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
            .state
            .profiles
            .len()
            .max(1);
        let (attempt_limit, _) = runtime_proxy_precommit_budget_for_profile_count(
            self.has_continuation_priority(),
            pressure_mode,
            profile_count,
        );
        Ok(selection_attempts >= attempt_limit
            || self.recovery_sweeps
                >= runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_SWEEP_LIMIT
            || self.recovery_started_at.is_some_and(|started_at| {
                started_at.elapsed()
                    >= std::time::Duration::from_millis(
                        runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS,
                    )
            }))
    }

    fn profile_count(&self) -> Result<usize> {
        Ok(self
            .shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
            .state
            .profiles
            .len()
            .max(1))
    }

    fn wait_for_transient_recovery(&mut self) -> Result<bool> {
        let provider_outage = self.saw_overload_failure && !self.has_continuation_priority();
        if !(self.saw_overload_failure || self.saw_transport_failure)
            || (!provider_outage
                && self.recovery_sweeps
                    >= runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_SWEEP_LIMIT)
        {
            return Ok(false);
        }
        let recovered = clear_runtime_recovered_profiles(
            self.shared,
            &mut self.excluded_profiles,
            RuntimeRouteKind::Websocket,
            true,
        )?;
        if recovered > 0 {
            self.recovery_sweeps = self.recovery_sweeps.saturating_add(1);
            runtime_proxy_log(
                self.shared,
                runtime_proxy_structured_log_message(
                    "rotation_sweep_start",
                    [
                        runtime_proxy_log_field("request", self.request_id.to_string()),
                        runtime_proxy_log_field("websocket_session", self.session_id.to_string()),
                        runtime_proxy_log_field(
                            "route",
                            runtime_route_kind_label(RuntimeRouteKind::Websocket),
                        ),
                        runtime_proxy_log_field("recovered_profiles", recovered.to_string()),
                        runtime_proxy_log_field("sweep", self.recovery_sweeps.to_string()),
                    ],
                ),
            );
            return Ok(true);
        }
        let Some(until) = runtime_profile_recovery_wait_for_route(
            self.shared,
            RuntimeRouteKind::Websocket,
            true,
        )?
        else {
            if provider_outage {
                let exponent = self.recovery_sweeps.min(5) as u32;
                let wait = Duration::from_millis(
                    250_u64
                        .saturating_mul(1_u64 << exponent)
                        .saturating_add((self.request_id + self.recovery_sweeps as u64) % 251)
                        .min(30_000),
                );
                runtime_proxy_log(
                    self.shared,
                    format!(
                        "request={} websocket_session={} provider_temporarily_unavailable_retry route=websocket wait_ms={} sweep={}",
                        self.request_id,
                        self.session_id,
                        wait.as_millis(),
                        self.recovery_sweeps.saturating_add(1),
                    ),
                );
                await_runtime_proxy_async_task(
                    self.shared,
                    "provider_recovery_wait",
                    async move {
                        tokio::time::sleep(wait).await;
                        Ok(())
                    },
                )?;
                self.excluded_profiles.clear();
                self.recovery_sweeps = self.recovery_sweeps.saturating_add(1);
                return Ok(true);
            }
            return Ok(false);
        };
        let recovery_started_at = *self
            .recovery_started_at
            .get_or_insert_with(std::time::Instant::now);
        let now = chrono::Local::now().timestamp();
        let recovery_budget = if provider_outage {
            Duration::from_secs(30)
        } else {
            Duration::from_millis(runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS)
                .saturating_sub(recovery_started_at.elapsed())
        };
        let wait =
            std::time::Duration::from_secs(u64::try_from(until.saturating_sub(now)).unwrap_or(0))
                .saturating_add(std::time::Duration::from_secs(1))
                .min(recovery_budget);
        let wait = if wait.is_zero() && provider_outage {
            Duration::from_millis(
                250_u64
                    .saturating_mul(1_u64 << self.recovery_sweeps.min(5) as u32)
                    .saturating_add((self.request_id + self.recovery_sweeps as u64) % 251)
                    .min(30_000),
            )
        } else {
            wait
        };
        if wait.is_zero() && !provider_outage {
            return Ok(false);
        }
        runtime_proxy_log(
            self.shared,
            runtime_proxy_structured_log_message(
                "rotation_waiting_for_recovery",
                [
                    runtime_proxy_log_field("request", self.request_id.to_string()),
                    runtime_proxy_log_field("websocket_session", self.session_id.to_string()),
                    runtime_proxy_log_field(
                        "route",
                        runtime_route_kind_label(RuntimeRouteKind::Websocket),
                    ),
                    runtime_proxy_log_field("wait_ms", wait.as_millis().to_string()),
                    runtime_proxy_log_field(
                        "sweep",
                        self.recovery_sweeps.saturating_add(1).to_string(),
                    ),
                ],
            ),
        );
        await_runtime_proxy_async_task(self.shared, "profile_recovery_wait", async move {
            tokio::time::sleep(wait).await;
            Ok(())
        })?;
        let recovered = clear_runtime_recovered_profiles(
            self.shared,
            &mut self.excluded_profiles,
            RuntimeRouteKind::Websocket,
            true,
        )?;
        self.recovery_sweeps = self.recovery_sweeps.saturating_add(1);
        runtime_proxy_log(
            self.shared,
            runtime_proxy_structured_log_message(
                "rotation_sweep_start",
                [
                    runtime_proxy_log_field("request", self.request_id.to_string()),
                    runtime_proxy_log_field("websocket_session", self.session_id.to_string()),
                    runtime_proxy_log_field(
                        "route",
                        runtime_route_kind_label(RuntimeRouteKind::Websocket),
                    ),
                    runtime_proxy_log_field("recovered_profiles", recovered.to_string()),
                    runtime_proxy_log_field("sweep", self.recovery_sweeps.to_string()),
                ],
            ),
        );
        Ok(recovered > 0)
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
        if self.wait_for_transient_recovery()? {
            return Ok(RuntimeWebsocketMessageLoopAction::Continue);
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
        if self.wait_for_transient_recovery()? {
            return Ok(RuntimeWebsocketMessageLoopAction::Continue);
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
            runtime_proxy_probe_refresh_pause(self.shared, RuntimeRouteKind::Websocket);
            return Ok(RuntimeWebsocketMessageLoopAction::Continue);
        }
        if self.try_luna_spark_fallback()? {
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
            runtime_smart_context_model_name_from_body(self.request_text.as_bytes()).as_deref(),
        )?
        else {
            return Ok(None);
        };
        let requested_model =
            runtime_smart_context_model_name_from_body(self.request_text.as_bytes());
        runtime_selection_trace_log_direct(
            self.shared,
            self.request_id,
            RuntimeSelectionTraceDirect {
                requested_model: requested_model.as_deref(),
                route_kind: RuntimeRouteKind::Websocket,
                candidate_key: &current_profile,
                class: runtime_proxy_crate::RuntimeRouteCandidateClass::Fallback,
                affinity_kind: None,
                hard_affinity: false,
            },
        );
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

    fn try_luna_spark_fallback(&mut self) -> Result<bool> {
        let Some(spark_model) = prodex_quota::openai_luna_spark_fallback_model(
            self.requested_model_name.as_deref(),
            runtime_smart_context_model_name_from_body(self.request_text.as_bytes()).as_deref(),
        ) else {
            return Ok(false);
        };
        if self.previous_response_id.is_some()
            || self.request_turn_state.is_some()
            || self.request_session_id.is_some()
            || self.request_requires_previous_response_affinity
            || self.prompt_cache_key.is_some()
            || self.has_continuation_priority()
            || self.saw_inflight_saturation
        {
            return Ok(false);
        }

        let Some(spark_candidate) = select_runtime_response_candidate_for_route_with_request(
            self.shared,
            RuntimeResponseCandidateSelection::fresh(
                &std::collections::BTreeSet::new(),
                RuntimeRouteKind::Websocket,
            ),
            Some(self.request_id),
            Some(spark_model),
        )?
        else {
            return Ok(false);
        };

        let rewritten = prodex_provider_core::provider_request_body_with_model(
            self.request_text.as_bytes(),
            spark_model,
        );
        self.request_text = String::from_utf8(rewritten)
            .map_err(|_| anyhow::anyhow!("Spark fallback request body was not valid UTF-8"))?;
        self.excluded_profiles.clear();
        self.last_failure = None;
        self.saw_transport_failure = false;
        self.saw_overload_failure = false;
        self.recovery_sweeps = 0;
        self.recovery_started_at = None;
        self.quota_last_chance_profile = Some(spark_candidate.clone());
        self.reset_selection_budget = true;
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} websocket_session={} model_fallback requested_model=luna effective_model={} profile={} reason=luna_capacity_unavailable_spark_available",
                self.request_id, self.session_id, spark_model, spark_candidate
            ),
        );
        Ok(true)
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
#[path = "../../../tests/src/runtime_proxy/websocket_message/loop_control.rs"]
mod tests;
