use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

use crate::RuntimeRouteKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProxyAdmissionRejection {
    GlobalLimit,
    LaneLimit(RuntimeRouteKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProxyQueueRejection {
    Full,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProxyGuardReleaseSnapshot {
    pub active_remaining: usize,
    pub lane_remaining: usize,
    pub active_underflow: bool,
    pub lane_underflow: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProfileInFlightReleaseSnapshot {
    pub remaining: usize,
    pub underflow: bool,
}

fn runtime_proxy_guarded_counter_release(counter: &AtomicUsize) -> (usize, bool) {
    loop {
        let current = counter.load(Ordering::SeqCst);
        if current == 0 {
            return (0, true);
        }
        let remaining = current - 1;
        if counter
            .compare_exchange(current, remaining, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return (remaining, false);
        }
    }
}

pub fn release_runtime_proxy_active_request_guard(
    active_request_count: &AtomicUsize,
    lane_active_count: &AtomicUsize,
    lane_releases_total: &AtomicU64,
    active_request_release_underflows_total: &AtomicU64,
    lane_release_underflows_total: &AtomicU64,
    wait: &(Mutex<()>, Condvar),
) -> RuntimeProxyGuardReleaseSnapshot {
    let (mutex, condvar) = wait;
    let _guard = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (active_remaining, active_underflow) =
        runtime_proxy_guarded_counter_release(active_request_count);
    let (lane_remaining, lane_underflow) = runtime_proxy_guarded_counter_release(lane_active_count);
    lane_releases_total.fetch_add(1, Ordering::Relaxed);
    if active_underflow {
        active_request_release_underflows_total.fetch_add(1, Ordering::Relaxed);
    }
    if lane_underflow {
        lane_release_underflows_total.fetch_add(1, Ordering::Relaxed);
    }
    condvar.notify_all();
    RuntimeProxyGuardReleaseSnapshot {
        active_remaining,
        lane_remaining,
        active_underflow,
        lane_underflow,
    }
}

pub fn runtime_proxy_background_queue_pressure_affects_route(route_kind: RuntimeRouteKind) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard
    )
}

pub fn runtime_proxy_pressure_mode_for_route(
    route_kind: RuntimeRouteKind,
    local_overload_pressure: bool,
    background_queue_pressure: bool,
) -> bool {
    local_overload_pressure
        || (background_queue_pressure
            && runtime_proxy_background_queue_pressure_affects_route(route_kind))
}

pub fn runtime_proxy_sync_probe_pressure_mode_for_route(
    route_kind: RuntimeRouteKind,
    local_overload_pressure: bool,
    background_queue_pressure: bool,
) -> bool {
    runtime_proxy_pressure_mode_for_route(
        route_kind,
        local_overload_pressure,
        background_queue_pressure,
    )
}

pub fn runtime_proxy_lane_limit_marks_global_overload(lane: RuntimeRouteKind) -> bool {
    lane == RuntimeRouteKind::Responses
}

pub fn runtime_proxy_should_shed_fresh_compact_request(
    pressure_mode: bool,
    session_profile: Option<&str>,
) -> bool {
    pressure_mode && session_profile.is_none()
}

#[cfg(test)]
#[path = "../tests/src/admission.rs"]
mod tests;
