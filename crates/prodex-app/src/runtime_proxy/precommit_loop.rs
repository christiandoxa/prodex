use super::{RuntimeRotationProxyShared, runtime_proxy_precommit_budget_exhausted_for_route};
use anyhow::Result;
use std::collections::BTreeSet;
use std::time::Instant;

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
    pub last_failure: Option<(F, bool)>,
}

impl<F> RuntimePrecommitLoopState<F> {
    pub fn new() -> Self {
        Self {
            selection_started_at: Instant::now(),
            selection_attempts: 0,
            excluded_profiles: BTreeSet::new(),
            saw_inflight_saturation: false,
            last_failure: None,
        }
    }

    pub fn budget_exhausted(
        &self,
        shared: &RuntimeRotationProxyShared,
        continuation: bool,
        pressure_mode: bool,
    ) -> Result<bool> {
        runtime_proxy_precommit_budget_exhausted_for_route(
            shared,
            self.selection_started_at,
            self.selection_attempts,
            continuation,
            pressure_mode,
        )
    }

    pub fn record_attempt(&mut self) {
        self.selection_attempts = self.selection_attempts.saturating_add(1);
    }

    pub fn record_inflight_saturation(&mut self) {
        self.saw_inflight_saturation = true;
    }

    pub fn restart_elapsed_budget(&mut self) {
        self.selection_started_at = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimePrecommitLoopState;

    #[test]
    fn attempt_and_elapsed_budget_state_reset_independently() {
        let mut state = RuntimePrecommitLoopState::<()>::new();
        state.record_attempt();
        let started_at = state.selection_started_at;
        state.restart_elapsed_budget();
        assert_eq!(state.selection_attempts, 1);
        assert!(state.selection_started_at >= started_at);
    }
}
