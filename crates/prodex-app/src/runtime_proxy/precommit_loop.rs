use super::{
    RuntimeRotationProxyShared, RuntimeRouteKind, await_runtime_proxy_async_task,
    clear_runtime_recovered_profiles, runtime_profile_recovery_wait_for_route, runtime_proxy_log,
    runtime_proxy_log_field, runtime_proxy_precommit_budget_exhausted_for_route,
    runtime_proxy_structured_log_message, runtime_route_has_retryable_profile,
    runtime_route_kind_label,
};
use anyhow::Result;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

pub(super) enum RuntimePrecommitLoopAction<C, R> {
    Continue,
    Attempt(C),
    Return(R),
}

pub(super) struct RuntimePrecommitLoopState<F> {
    pub selection_started_at: Instant,
    pub selection_attempts: usize,
    pub excluded_profiles: BTreeSet<String>,
    pub saw_inflight_saturation: bool,
    pub saw_transport_failure: bool,
    pub saw_transport_recovery_candidate: bool,
    pub saw_overload_failure: bool,
    pub saw_rate_limit_failure: bool,
    pub cold_start_probe_waited: bool,
    pub recovery_sweeps: usize,
    pub last_failure: Option<(F, bool)>,
}

impl<F> RuntimePrecommitLoopState<F> {
    pub fn new() -> Self {
        Self {
            selection_started_at: Instant::now(),
            selection_attempts: 0,
            excluded_profiles: BTreeSet::new(),
            saw_inflight_saturation: false,
            saw_transport_failure: false,
            saw_transport_recovery_candidate: false,
            saw_overload_failure: false,
            saw_rate_limit_failure: false,
            cold_start_probe_waited: false,
            recovery_sweeps: 0,
            last_failure: None,
        }
    }

    pub fn budget_exhausted(
        &self,
        shared: &RuntimeRotationProxyShared,
        route_kind: RuntimeRouteKind,
        continuation: bool,
        pressure_mode: bool,
    ) -> Result<bool> {
        let normal_budget_exhausted = runtime_proxy_precommit_budget_exhausted_for_route(
            shared,
            self.selection_started_at,
            self.selection_attempts,
            continuation,
            pressure_mode,
        )?;
        if self.saw_transient_failure() && runtime_route_has_retryable_profile(shared, route_kind)?
        {
            return Ok(false);
        }
        if self.recovery_sweeps == 0 {
            if self.selection_attempts < Self::profile_count(shared)? {
                return Ok(false);
            }
            return Ok(normal_budget_exhausted);
        }
        let profile_count = Self::profile_count(shared)?;
        let (attempt_limit, _) =
            runtime_proxy_crate::runtime_proxy_precommit_budget_for_profile_count(
                continuation,
                pressure_mode,
                profile_count,
            );
        Ok(self.selection_attempts >= attempt_limit)
    }

    fn profile_count(shared: &RuntimeRotationProxyShared) -> Result<usize> {
        Ok(shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
            .state
            .profiles
            .len()
            .max(1))
    }

    pub fn record_attempt(&mut self) {
        self.selection_attempts = self.selection_attempts.saturating_add(1);
    }

    pub fn claim_cold_start_probe_wait(&mut self) -> bool {
        if self.cold_start_probe_waited {
            return false;
        }
        self.cold_start_probe_waited = true;
        true
    }

    pub fn record_inflight_saturation(&mut self) {
        self.saw_inflight_saturation = true;
    }

    pub fn record_transport_failure_at(&mut self, stage: &str) {
        self.saw_transport_failure = true;
        self.saw_transport_recovery_candidate |=
            !stage.contains("upstream_request") && !stage.contains("connect");
    }

    pub fn record_overload_failure(&mut self) {
        self.saw_overload_failure = true;
    }

    pub fn record_rate_limit_failure(&mut self) {
        self.saw_rate_limit_failure = true;
    }

    fn saw_transient_failure(&self) -> bool {
        self.saw_overload_failure || self.saw_rate_limit_failure || self.saw_transport_failure
    }

    pub fn record_recovery_sweep(&mut self) {
        self.recovery_sweeps = self.recovery_sweeps.saturating_add(1);
    }

    pub fn maybe_wait_for_transient_recovery(
        &mut self,
        request_id: u64,
        shared: &RuntimeRotationProxyShared,
        route_kind: RuntimeRouteKind,
    ) -> Result<bool> {
        if !self.saw_transient_failure()
            || !runtime_route_has_retryable_profile(shared, route_kind)?
        {
            return Ok(false);
        }
        let recovered = clear_runtime_recovered_profiles(
            shared,
            &mut self.excluded_profiles,
            route_kind,
            true,
        )?;
        if recovered > 0 {
            self.record_recovery_sweep();
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "rotation_sweep_start",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("recovered_profiles", recovered.to_string()),
                        runtime_proxy_log_field("sweep", self.recovery_sweeps.to_string()),
                    ],
                ),
            );
            return Ok(true);
        }
        let Some(wait) = runtime_profile_recovery_wait_for_route(shared, route_kind, true)?
            .map(|until| {
                let now = chrono::Local::now().timestamp();
                Duration::from_secs(u64::try_from(until.saturating_sub(now)).unwrap_or(0))
                    .saturating_add(Duration::from_secs(1))
                    .min(Duration::from_secs(30))
            })
            .filter(|wait| !wait.is_zero())
        else {
            if self.saw_overload_failure {
                let exponent = self.recovery_sweeps.min(5) as u32;
                let base_ms = 250_u64.saturating_mul(1_u64 << exponent);
                let jitter_ms = (request_id.saturating_add(self.recovery_sweeps as u64)) % 251;
                let wait = Duration::from_millis(base_ms.saturating_add(jitter_ms).min(30_000));
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "provider_temporarily_unavailable_retry",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("wait_ms", wait.as_millis().to_string()),
                            runtime_proxy_log_field(
                                "sweep",
                                self.recovery_sweeps.saturating_add(1).to_string(),
                            ),
                        ],
                    ),
                );
                await_runtime_proxy_async_task(shared, "provider_recovery_wait", async move {
                    tokio::time::sleep(wait).await;
                    Ok(())
                })?;
                let recovered = clear_runtime_recovered_profiles(
                    shared,
                    &mut self.excluded_profiles,
                    route_kind,
                    true,
                )?;
                self.record_recovery_sweep();
                return Ok(recovered > 0 || self.saw_overload_failure);
            }
            return Ok(false);
        };

        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "rotation_waiting_for_recovery",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                    runtime_proxy_log_field("wait_ms", wait.as_millis().to_string()),
                    runtime_proxy_log_field(
                        "sweep",
                        self.recovery_sweeps.saturating_add(1).to_string(),
                    ),
                ],
            ),
        );
        await_runtime_proxy_async_task(shared, "profile_recovery_wait", async move {
            tokio::time::sleep(wait).await;
            Ok(())
        })?;
        let recovered = clear_runtime_recovered_profiles(
            shared,
            &mut self.excluded_profiles,
            route_kind,
            true,
        )?;
        self.record_recovery_sweep();
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "rotation_sweep_start",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                    runtime_proxy_log_field("recovered_profiles", recovered.to_string()),
                    runtime_proxy_log_field("sweep", self.recovery_sweeps.to_string()),
                ],
            ),
        );
        Ok(recovered > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimePrecommitLoopState;

    #[test]
    fn transport_failures_are_recoverable_transient_failures() {
        let mut state = RuntimePrecommitLoopState::<()>::new();

        assert!(!state.saw_transient_failure());
        state.record_transport_failure_at("response_body");
        assert!(state.saw_transient_failure());
    }

    #[test]
    fn transport_recovery_candidate_survives_later_connect_failure() {
        let mut state = RuntimePrecommitLoopState::<()>::new();

        state.record_transport_failure_at("responses_sse_lookahead");
        state.record_transport_failure_at("responses_upstream_request");

        assert!(state.saw_transient_failure());
    }

    #[test]
    fn recovery_sweeps_track_retry_epochs_without_becoming_a_terminal_cap() {
        let mut state = RuntimePrecommitLoopState::<()>::new();

        state.record_overload_failure();
        for _ in 0..8 {
            state.record_recovery_sweep();
        }

        assert_eq!(state.recovery_sweeps, 8);
        assert!(state.saw_transient_failure());
    }

    #[test]
    fn attempts_share_one_elapsed_budget() {
        let mut state = RuntimePrecommitLoopState::<()>::new();
        let started_at = state.selection_started_at;
        state.record_attempt();
        state.record_attempt();
        assert_eq!(state.selection_attempts, 2);
        assert_eq!(state.selection_started_at, started_at);
    }
}
