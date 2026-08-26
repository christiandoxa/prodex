use super::{
    RuntimeRotationProxyShared, RuntimeRouteKind, await_runtime_proxy_async_task,
    clear_runtime_recovered_profiles, runtime_profile_recovery_wait_for_route, runtime_proxy_log,
    runtime_proxy_log_field, runtime_proxy_precommit_budget_exhausted_for_route,
    runtime_proxy_structured_log_message, runtime_route_kind_label,
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
    pub saw_overload_failure: bool,
    pub recovery_sweeps: usize,
    pub recovery_started_at: Option<Instant>,
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
            saw_overload_failure: false,
            recovery_sweeps: 0,
            recovery_started_at: None,
            last_failure: None,
        }
    }

    pub fn budget_exhausted(
        &self,
        shared: &RuntimeRotationProxyShared,
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
        if self.recovery_sweeps == 0 {
            if self.saw_transport_failure && self.selection_attempts < Self::profile_count(shared)?
            {
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
        Ok(self.selection_attempts >= attempt_limit || self.recovery_budget_exhausted())
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

    pub fn record_inflight_saturation(&mut self) {
        self.saw_inflight_saturation = true;
    }

    pub fn record_transport_failure(&mut self) {
        self.saw_transport_failure = true;
    }

    pub fn record_overload_failure(&mut self) {
        self.saw_overload_failure = true;
    }

    pub fn recovery_budget_exhausted(&self) -> bool {
        self.recovery_sweeps >= runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_SWEEP_LIMIT
            || self.recovery_started_at.is_some_and(|started_at| {
                started_at.elapsed()
                    >= Duration::from_millis(
                        runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS,
                    )
            })
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
        if !self.saw_overload_failure || self.recovery_budget_exhausted() {
            return Ok(false);
        }
        let recovery_started_at = *self.recovery_started_at.get_or_insert_with(Instant::now);
        let recovery_budget =
            Duration::from_millis(runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS)
                .saturating_sub(recovery_started_at.elapsed());
        let Some(wait) = runtime_profile_recovery_wait_for_route(shared, route_kind, true)?
            .map(|until| {
                let now = chrono::Local::now().timestamp();
                Duration::from_secs(u64::try_from(until.saturating_sub(now)).unwrap_or(0))
                    .saturating_add(Duration::from_secs(1))
                    .min(recovery_budget)
            })
            .filter(|wait| !wait.is_zero())
        else {
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
    fn attempts_share_one_elapsed_budget() {
        let mut state = RuntimePrecommitLoopState::<()>::new();
        let started_at = state.selection_started_at;
        state.record_attempt();
        state.record_attempt();
        assert_eq!(state.selection_attempts, 2);
        assert_eq!(state.selection_started_at, started_at);
    }
}
