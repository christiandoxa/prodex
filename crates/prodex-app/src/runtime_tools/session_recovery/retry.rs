//! Monotonic host bookkeeping for the Mojo-owned no-progress retry schedule.
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(super) struct NativeRecoveryRetry {
    session: Option<String>,
    progress: u64,
    failure: Option<u64>,
    pub(super) attempts: u32,
    pub(super) next_attempt_at: Instant,
}

impl NativeRecoveryRetry {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            session: None,
            progress: 0,
            failure: None,
            attempts: 0,
            next_attempt_at: now,
        }
    }

    pub(super) fn ready(
        &mut self,
        session: &str,
        progress: u64,
        failure: u64,
        now: Instant,
    ) -> bool {
        if self.session.as_deref() != Some(session) || self.progress != progress {
            self.session = Some(session.to_owned());
            self.progress = progress;
            self.failure = None;
            self.attempts = 0;
            self.next_attempt_at = now;
        }
        if self.failure != Some(failure) {
            self.failure = Some(failure);
            // Start the cooldown at this failure, never at the previous RPC.
            self.next_attempt_at = now + self.delay();
        }
        now >= self.next_attempt_at
    }

    pub(super) fn completed_attempt(&mut self, now: Instant) {
        self.attempts = self.attempts.saturating_add(1);
        self.next_attempt_at = now + self.delay();
    }

    fn delay(&self) -> Duration {
        Duration::from_millis(
            prodex_mojo_core::launch::recovery_retry_delay_ms(self.attempts)
                .expect("Mojo native recovery retry delay must be valid"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_retry_and_long_timeout_do_not_reset_or_consume_backoff() {
        let now = Instant::now();
        let mut retry = NativeRecoveryRetry::new(now);
        assert!(retry.ready("root", 0, 1, now));
        retry.completed_attempt(now);
        let failed_at = now + Duration::from_secs(96);
        assert!(!retry.ready("root", 0, 2, failed_at));
        assert!(!retry.ready("root", 0, 2, failed_at + Duration::from_secs(4)));
        assert!(retry.ready("root", 0, 2, failed_at + Duration::from_secs(5)));
        retry.completed_attempt(failed_at + Duration::from_secs(5));
        assert_eq!(retry.attempts, 2, "an accepted control RPC is not progress");
        let third = failed_at + Duration::from_secs(101);
        assert!(!retry.ready("root", 0, 3, third));
        assert_eq!(retry.next_attempt_at, third + Duration::from_secs(10));
        assert!(
            retry.ready("root", 1, 3, third),
            "real progress resets accumulated retry delay"
        );
        assert_eq!(retry.attempts, 0);
    }

    #[test]
    fn unavailable_control_plane_backs_off_without_new_terminal_events() {
        let now = Instant::now();
        let mut retry = NativeRecoveryRetry::new(now);
        assert!(retry.ready("root", 0, 1, now));
        retry.completed_attempt(now);
        assert!(!retry.ready("root", 0, 1, now + Duration::from_secs(4)));
        assert!(retry.ready("root", 0, 1, now + Duration::from_secs(5)));
        retry.completed_attempt(now + Duration::from_secs(5));
        assert_eq!(retry.next_attempt_at, now + Duration::from_secs(15));
        assert!(
            retry.ready("another-root", 0, 1, now),
            "another session must not inherit a circuit"
        );
        assert_eq!(retry.attempts, 0);
    }
}
