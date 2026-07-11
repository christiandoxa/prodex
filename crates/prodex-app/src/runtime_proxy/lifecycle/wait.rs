//! Runtime proxy wait outcome helpers.

use super::*;

pub(crate) fn runtime_profile_inflight_release_revision(
    shared: &RuntimeRotationProxyShared,
) -> u64 {
    shared
        .lane_admission
        .inflight_release_revision
        .load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeProfileInFlightWaitOutcome {
    InflightRelease,
    OtherNotify,
    Timeout,
}

pub(crate) fn runtime_profile_wait_outcome_label(
    outcome: RuntimeProfileInFlightWaitOutcome,
) -> &'static str {
    match outcome {
        RuntimeProfileInFlightWaitOutcome::InflightRelease => "release",
        RuntimeProfileInFlightWaitOutcome::OtherNotify => "other_notify",
        RuntimeProfileInFlightWaitOutcome::Timeout => "timeout",
    }
}

pub(crate) fn runtime_profile_inflight_wait_outcome_label(
    outcome: RuntimeProfileInFlightWaitOutcome,
) -> &'static str {
    match outcome {
        RuntimeProfileInFlightWaitOutcome::InflightRelease => "inflight_release",
        RuntimeProfileInFlightWaitOutcome::OtherNotify => "other_notify",
        RuntimeProfileInFlightWaitOutcome::Timeout => "timeout",
    }
}

pub(crate) fn runtime_profile_inflight_wait_outcome_since(
    shared: &RuntimeRotationProxyShared,
    timeout: Duration,
    observed_revision: u64,
) -> RuntimeProfileInFlightWaitOutcome {
    if timeout.is_zero() {
        return RuntimeProfileInFlightWaitOutcome::Timeout;
    }
    if runtime_profile_inflight_release_revision(shared) != observed_revision {
        return RuntimeProfileInFlightWaitOutcome::InflightRelease;
    }
    let (mutex, condvar) = &*shared.lane_admission.wait;
    let guard = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if runtime_profile_inflight_release_revision(shared) != observed_revision {
        return RuntimeProfileInFlightWaitOutcome::InflightRelease;
    }
    let (_guard, result) = condvar
        .wait_timeout(guard, timeout)
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if runtime_profile_inflight_release_revision(shared) != observed_revision {
        RuntimeProfileInFlightWaitOutcome::InflightRelease
    } else if result.timed_out() {
        RuntimeProfileInFlightWaitOutcome::Timeout
    } else {
        RuntimeProfileInFlightWaitOutcome::OtherNotify
    }
}

pub(crate) fn runtime_probe_refresh_wait_outcome_since(
    timeout: Duration,
    observed_revision: u64,
) -> RuntimeProfileInFlightWaitOutcome {
    if timeout.is_zero() {
        return RuntimeProfileInFlightWaitOutcome::Timeout;
    }
    if runtime_probe_refresh_revision() != observed_revision {
        return RuntimeProfileInFlightWaitOutcome::InflightRelease;
    }
    let queue = runtime_probe_refresh_queue();
    let (mutex, condvar) = &*queue.wait;
    let guard = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if runtime_probe_refresh_revision() != observed_revision {
        return RuntimeProfileInFlightWaitOutcome::InflightRelease;
    }
    let (_guard, result) = condvar
        .wait_timeout(guard, timeout)
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if runtime_probe_refresh_revision() != observed_revision {
        RuntimeProfileInFlightWaitOutcome::InflightRelease
    } else if result.timed_out() {
        RuntimeProfileInFlightWaitOutcome::Timeout
    } else {
        RuntimeProfileInFlightWaitOutcome::OtherNotify
    }
}

#[cfg(test)]
pub(crate) fn wait_for_runtime_profile_inflight_relief_since(
    shared: &RuntimeRotationProxyShared,
    timeout: Duration,
    observed_revision: u64,
) -> bool {
    matches!(
        runtime_profile_inflight_wait_outcome_since(shared, timeout, observed_revision),
        RuntimeProfileInFlightWaitOutcome::InflightRelease
    )
}

#[cfg(test)]
pub(crate) fn wait_for_runtime_probe_refresh_since(
    timeout: Duration,
    observed_revision: u64,
) -> bool {
    matches!(
        runtime_probe_refresh_wait_outcome_since(timeout, observed_revision),
        RuntimeProfileInFlightWaitOutcome::InflightRelease
    )
}
