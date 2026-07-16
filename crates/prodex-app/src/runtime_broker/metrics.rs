use anyhow::Result;
use chrono::Local;
use prodex_runtime_broker::{
    RuntimeBrokerAllocationMetrics, RuntimeBrokerContinuityFailureReasonMetrics,
    RuntimeBrokerLaneMetrics, RuntimeBrokerMetadata, RuntimeBrokerMetrics,
    RuntimeBrokerTrafficMetrics, runtime_broker_continuity_failure_reason_metrics_with_live,
};
#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::fs;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::atomic::Ordering;
#[cfg(test)]
use std::sync::atomic::{AtomicU64, AtomicUsize};
#[cfg(test)]
use std::sync::{Arc, Mutex};

use crate::{
    RUNTIME_CONTINUATION_VERIFIED_STALE_SECONDS, RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS,
    RUNTIME_PROFILE_HEALTH_DECAY_SECONDS, RuntimeRotationProxyShared, RuntimeRouteKind,
    runtime_proxy_continuity_failure_reason_metrics_snapshot, runtime_proxy_persistence_enabled,
};

#[cfg(test)]
use crate::{
    AppPaths, AppState, ProfileEntry, ProfileProvider, RuntimeContinuationStatuses,
    RuntimeProxyLaneAdmission, RuntimeRotationState, acquire_test_runtime_lock,
    clear_all_runtime_proxy_continuity_failure_reason_metrics,
    clear_runtime_proxy_continuity_failure_reason_metrics,
    runtime_proxy_continuity_failure_reason_metrics_store_entry_count, runtime_proxy_lane_limits,
    runtime_proxy_record_continuity_failure_reason,
};

fn runtime_broker_continuity_failure_reason_metrics(
    log_path: &Path,
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    prodex_runtime_broker_log::runtime_broker_cached_continuity_failure_reason_metrics(log_path)
}

fn runtime_broker_live_continuity_failure_reason_metrics(
    log_path: &Path,
    parsed_metrics: &RuntimeBrokerContinuityFailureReasonMetrics,
) -> Option<RuntimeBrokerContinuityFailureReasonMetrics> {
    let snapshot = runtime_proxy_continuity_failure_reason_metrics_snapshot(log_path)?;
    Some(runtime_broker_continuity_failure_reason_metrics_with_live(
        parsed_metrics.clone(),
        &snapshot.baseline_metrics,
        snapshot.live_metrics,
    ))
}

fn runtime_broker_live_lane_metrics(
    shared: &RuntimeRotationProxyShared,
    lane: RuntimeRouteKind,
) -> RuntimeBrokerLaneMetrics {
    RuntimeBrokerLaneMetrics {
        active: shared
            .lane_admission
            .active_counter(lane)
            .load(Ordering::SeqCst),
        limit: shared.lane_admission.limit(lane),
        admissions_total: shared
            .lane_admission
            .admissions_total_counter(lane)
            .load(Ordering::Relaxed),
        releases_total: shared
            .lane_admission
            .releases_total_counter(lane)
            .load(Ordering::Relaxed),
        global_limit_rejections_total: shared
            .lane_admission
            .global_limit_rejections_total_counter(lane)
            .load(Ordering::Relaxed),
        lane_limit_rejections_total: shared
            .lane_admission
            .lane_limit_rejections_total_counter(lane)
            .load(Ordering::Relaxed),
        release_underflows_total: shared
            .lane_admission
            .release_underflows_total_counter(lane)
            .load(Ordering::Relaxed),
    }
}

fn runtime_broker_allocation_metrics() -> Option<RuntimeBrokerAllocationMetrics> {
    #[cfg(feature = "allocation-bench-support")]
    {
        let snapshot = crate::allocation_bench_support::runtime_allocation_snapshot();
        Some(RuntimeBrokerAllocationMetrics {
            alloc_calls: snapshot.alloc_calls,
            realloc_calls: snapshot.realloc_calls,
            dealloc_calls: snapshot.dealloc_calls,
            allocated_bytes: snapshot.allocated_bytes,
            reallocated_bytes: snapshot.reallocated_bytes,
            deallocated_bytes: snapshot.deallocated_bytes,
            live_bytes: snapshot.live_bytes,
            peak_live_bytes: snapshot.peak_live_bytes,
        })
    }

    #[cfg(not(feature = "allocation-bench-support"))]
    None
}

pub(crate) fn runtime_broker_metrics_snapshot(
    shared: &RuntimeRotationProxyShared,
    metadata: &RuntimeBrokerMetadata,
) -> Result<RuntimeBrokerMetrics> {
    let now = Local::now().timestamp();
    let now_u64 = now.max(0) as u64;
    let parsed_continuity_failure_reasons =
        runtime_broker_continuity_failure_reason_metrics(&shared.log_path);
    let continuity_failure_reasons = runtime_broker_live_continuity_failure_reason_metrics(
        &shared.log_path,
        &parsed_continuity_failure_reasons,
    )
    .unwrap_or(parsed_continuity_failure_reasons);
    let profile_inflight = shared.lane_admission.profile_inflight_snapshot();
    let runtime = shared
        .lock_runtime_state()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;

    Ok(
        prodex_runtime_broker::runtime_broker_metrics_from_snapshot_input(
            prodex_runtime_broker::RuntimeBrokerMetricsSnapshotInput {
                metadata,
                pid: std::process::id(),
                active_requests: shared.active_request_count.load(Ordering::SeqCst),
                persistence_owner: runtime_proxy_persistence_enabled(shared),
                active_request_limit: shared.active_request_limit,
                local_overload_backoff_remaining_seconds: shared
                    .local_overload_backoff_until
                    .load(Ordering::SeqCst)
                    .saturating_sub(now_u64),
                runtime_state_lock_wait: shared.runtime_state_lock_wait_metrics(),
                admission_wait: shared
                    .lane_admission
                    .admission_wait_metric_counters()
                    .snapshot(),
                long_lived_queue_wait: shared
                    .lane_admission
                    .long_lived_queue_wait_metric_counters()
                    .snapshot(),
                allocation: runtime_broker_allocation_metrics(),
                traffic: RuntimeBrokerTrafficMetrics {
                    responses: runtime_broker_live_lane_metrics(
                        shared,
                        RuntimeRouteKind::Responses,
                    ),
                    compact: runtime_broker_live_lane_metrics(shared, RuntimeRouteKind::Compact),
                    websocket: runtime_broker_live_lane_metrics(
                        shared,
                        RuntimeRouteKind::Websocket,
                    ),
                    standard: runtime_broker_live_lane_metrics(shared, RuntimeRouteKind::Standard),
                },
                profile_inflight: &profile_inflight,
                profile_retry_backoff_until: &runtime.profile_retry_backoff_until,
                profile_transport_backoff_until: &runtime.profile_transport_backoff_until,
                profile_route_circuit_open_until: &runtime.profile_route_circuit_open_until,
                profile_health: &runtime.profile_health,
                continuation_statuses: &runtime.continuation_statuses,
                continuity_failure_reasons,
                now,
                health_decay_seconds: RUNTIME_PROFILE_HEALTH_DECAY_SECONDS,
                stale_verified_seconds: RUNTIME_CONTINUATION_VERIFIED_STALE_SECONDS,
                previous_response_negative_cache_seconds:
                    RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS,
            },
        )
        .with_guard_counters(prodex_runtime_broker::RuntimeBrokerMetricsGuardCounters {
            active_request_release_underflows_total: shared
                .lane_admission
                .active_request_release_underflows_total(),
            profile_inflight_admissions_total: shared
                .lane_admission
                .profile_inflight_admissions_total(),
            profile_inflight_releases_total: shared
                .lane_admission
                .profile_inflight_releases_total(),
            profile_inflight_release_underflows_total: shared
                .lane_admission
                .profile_inflight_release_underflows_total(),
        }),
    )
}

#[cfg(test)]
#[path = "../../tests/src/runtime_broker/metrics.rs"]
mod tests;
