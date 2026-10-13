//! A request-scoped precommit watchdog. It never modifies context limits,
//! resets a committed stream, or clears account/session ownership.
use super::{RuntimeRotationProxyShared, runtime_proxy_log};
use anyhow::Result;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) struct RuntimeProxyRecoveryTimeout;

impl std::fmt::Display for RuntimeProxyRecoveryTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("request timed out while waiting for upstream recovery before output started")
    }
}
impl std::error::Error for RuntimeProxyRecoveryTimeout {}

impl RuntimeRotationProxyShared {
    pub(crate) fn for_precommit_request(&self) -> Self {
        let mut scoped = self.clone();
        if scoped.precommit_deadline.is_none() {
            let budget = prodex_mojo_core::runtime::recovery_watchdog_budget_ms(
                self.runtime_config.tuning.stream_idle_timeout_ms,
                runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS,
            )
            .expect("Mojo recovery watchdog budget must be positive");
            let now = Instant::now();
            scoped.precommit_deadline = Some(
                now.checked_add(Duration::from_millis(budget))
                    .unwrap_or(now),
            );
        }
        scoped
    }

    pub(crate) fn check_precommit_deadline(&self) -> Result<()> {
        if self
            .precommit_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            runtime_proxy_log(
                self,
                "precommit_recovery_deadline_exceeded stream_committed=false",
            );
            return Err(RuntimeProxyRecoveryTimeout.into());
        }
        Ok(())
    }

    pub(crate) fn precommit_wait_slice(&self, requested: Duration) -> Result<Duration> {
        self.check_precommit_deadline()?;
        Ok(self.precommit_deadline.map_or(requested, |deadline| {
            requested.min(deadline.saturating_duration_since(Instant::now()))
        }))
    }
}
