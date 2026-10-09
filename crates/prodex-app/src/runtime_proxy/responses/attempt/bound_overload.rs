use super::*;

/// Retry a precommit overload without moving a continuation to another owner.
/// The fixed attempt/deadline/backoff policy is implemented in Mojo.
pub(super) struct BoundOverloadRetry<'a> {
    shared: &'a RuntimeRotationProxyShared,
    profile_name: &'a str,
    request_id: u64,
    hard_affinity: bool,
    previous_response_present: bool,
    started_at: Instant,
    retries: usize,
}

impl<'a> BoundOverloadRetry<'a> {
    pub(super) fn new(
        shared: &'a RuntimeRotationProxyShared,
        profile_name: &'a str,
        request_id: u64,
        hard_affinity: bool,
        previous_response_present: bool,
    ) -> Self {
        Self {
            shared,
            profile_name,
            request_id,
            hard_affinity,
            previous_response_present,
            started_at: Instant::now(),
            retries: 0,
        }
    }

    pub(super) fn retain_or_retry(
        &mut self,
        attempt: Option<RuntimeResponsesAttempt>,
        inflight_guard: &mut Option<RuntimeProfileInFlightGuard>,
    ) -> Result<Option<RuntimeResponsesAttempt>> {
        let Some(attempt) = attempt else {
            return Ok(None);
        };
        if self.maybe_retry(&attempt, inflight_guard)? {
            Ok(None)
        } else {
            Ok(Some(attempt))
        }
    }

    fn maybe_retry(
        &mut self,
        attempt: &RuntimeResponsesAttempt,
        inflight_guard: &mut Option<RuntimeProfileInFlightGuard>,
    ) -> Result<bool> {
        let RuntimeResponsesAttempt::Overloaded { retry_after, .. } = attempt else {
            return Ok(false);
        };
        let delay = prodex_mojo_core::runtime::bound_overload_retry_delay(
            self.hard_affinity,
            self.previous_response_present,
            false,
            self.retries,
            self.started_at.elapsed(),
            *retry_after,
            self.request_id,
        )
        .map_err(|error| anyhow::anyhow!("Mojo bound-overload retry policy failed: {error:?}"))?;
        let Some(delay) = delay else {
            return Ok(false);
        };
        // Failed payloads can be retained, but must not hold admission capacity.
        drop(inflight_guard.take());
        self.retries += 1;
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} transport=http bound_overload_retry profile={} retry={} delay_ms={} committed=false affinity_preserved=true",
                self.request_id,
                self.profile_name,
                self.retries,
                delay.as_millis(),
            ),
        );
        await_runtime_proxy_async_task(self.shared, "responses_bound_overload_wait", async move {
            tokio::time::sleep(delay).await;
            Ok(())
        })?;
        *inflight_guard = try_acquire_runtime_profile_inflight_guard(
            self.shared,
            self.profile_name,
            "responses_http",
            true,
        )?;
        Ok(inflight_guard.is_some())
    }
}
