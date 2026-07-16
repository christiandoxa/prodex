use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeRouteKind {
    Responses,
    Compact,
    Websocket,
    Standard,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeWaitDurationMetrics {
    pub wait_total_ns: u64,
    pub wait_count: u64,
    pub wait_max_ns: u64,
}

#[derive(Debug, Default)]
pub struct RuntimeWaitDurationMetricCounters {
    wait_total_ns: AtomicU64,
    wait_count: AtomicU64,
    wait_max_ns: AtomicU64,
}

impl RuntimeWaitDurationMetricCounters {
    pub fn record_wait(&self, wait: Duration) {
        let wait_ns = wait.as_nanos().min(u128::from(u64::MAX)) as u64;
        self.wait_total_ns.fetch_add(wait_ns, Ordering::Relaxed);
        self.wait_count.fetch_add(1, Ordering::Relaxed);
        let mut current_max = self.wait_max_ns.load(Ordering::Relaxed);
        while wait_ns > current_max {
            match self.wait_max_ns.compare_exchange_weak(
                current_max,
                wait_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current_max = observed,
            }
        }
    }

    pub fn snapshot(&self) -> RuntimeWaitDurationMetrics {
        RuntimeWaitDurationMetrics {
            wait_total_ns: self.wait_total_ns.load(Ordering::Relaxed),
            wait_count: self.wait_count.load(Ordering::Relaxed),
            wait_max_ns: self.wait_max_ns.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.wait_total_ns.store(0, Ordering::Relaxed);
        self.wait_count.store(0, Ordering::Relaxed);
        self.wait_max_ns.store(0, Ordering::Relaxed);
    }
}

/// Compatibility name retained for runtime-state lock metrics consumers.
pub type RuntimeStateLockWaitMetrics = RuntimeWaitDurationMetrics;
/// Compatibility name retained for runtime-state lock counter consumers.
pub type RuntimeStateLockWaitMetricCounters = RuntimeWaitDurationMetricCounters;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeProxyLaneLimits {
    pub responses: usize,
    pub compact: usize,
    pub websocket: usize,
    pub standard: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProxyAdmissionLimit {
    Global { active: usize, limit: usize },
    Lane { active: usize, limit: usize },
}

#[derive(Debug)]
pub struct RuntimeProxyAdmissionPermit {
    active_request_count: Arc<AtomicUsize>,
    lane_active_count: Arc<AtomicUsize>,
    lane_releases_total: Arc<AtomicU64>,
    active_request_release_underflows_total: Arc<AtomicU64>,
    lane_release_underflows_total: Arc<AtomicU64>,
    wait: Arc<(Mutex<()>, Condvar)>,
}

#[derive(Debug)]
pub struct RuntimeProxyAdmissionAcquired {
    pub permit: RuntimeProxyAdmissionPermit,
    pub bypassed_lane_limit: bool,
}

fn release_counter(counter: &AtomicUsize) -> bool {
    loop {
        let current = counter.load(Ordering::SeqCst);
        if current == 0 {
            return true;
        }
        if counter
            .compare_exchange(current, current - 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return false;
        }
    }
}

impl Drop for RuntimeProxyAdmissionPermit {
    fn drop(&mut self) {
        let (mutex, condvar) = &*self.wait;
        let _guard = mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let active_underflow = release_counter(&self.active_request_count);
        let lane_underflow = release_counter(&self.lane_active_count);
        self.lane_releases_total.fetch_add(1, Ordering::Relaxed);
        if active_underflow {
            self.active_request_release_underflows_total
                .fetch_add(1, Ordering::Relaxed);
        }
        if lane_underflow {
            self.lane_release_underflows_total
                .fetch_add(1, Ordering::Relaxed);
        }
        condvar.notify_all();
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeProxyLaneAdmission {
    responses_active: Arc<AtomicUsize>,
    compact_active: Arc<AtomicUsize>,
    websocket_active: Arc<AtomicUsize>,
    standard_active: Arc<AtomicUsize>,
    responses_admissions_total: Arc<AtomicU64>,
    compact_admissions_total: Arc<AtomicU64>,
    websocket_admissions_total: Arc<AtomicU64>,
    standard_admissions_total: Arc<AtomicU64>,
    responses_releases_total: Arc<AtomicU64>,
    compact_releases_total: Arc<AtomicU64>,
    websocket_releases_total: Arc<AtomicU64>,
    standard_releases_total: Arc<AtomicU64>,
    responses_global_limit_rejections_total: Arc<AtomicU64>,
    compact_global_limit_rejections_total: Arc<AtomicU64>,
    websocket_global_limit_rejections_total: Arc<AtomicU64>,
    standard_global_limit_rejections_total: Arc<AtomicU64>,
    responses_lane_limit_rejections_total: Arc<AtomicU64>,
    compact_lane_limit_rejections_total: Arc<AtomicU64>,
    websocket_lane_limit_rejections_total: Arc<AtomicU64>,
    standard_lane_limit_rejections_total: Arc<AtomicU64>,
    active_request_release_underflows_total: Arc<AtomicU64>,
    responses_release_underflows_total: Arc<AtomicU64>,
    compact_release_underflows_total: Arc<AtomicU64>,
    websocket_release_underflows_total: Arc<AtomicU64>,
    standard_release_underflows_total: Arc<AtomicU64>,
    profile_inflight_admissions_total: Arc<AtomicU64>,
    profile_inflight_releases_total: Arc<AtomicU64>,
    profile_inflight_release_underflows_total: Arc<AtomicU64>,
    profile_inflight: Arc<Mutex<BTreeMap<String, usize>>>,
    admission_wait_metrics: Arc<RuntimeWaitDurationMetricCounters>,
    long_lived_queue_wait_metrics: Arc<RuntimeWaitDurationMetricCounters>,
    wait: Arc<(Mutex<()>, Condvar)>,
    inflight_release_revision: Arc<AtomicU64>,
    limits: RuntimeProxyLaneLimits,
}

impl RuntimeProxyLaneAdmission {
    pub fn new(limits: RuntimeProxyLaneLimits) -> Self {
        Self {
            responses_active: Arc::new(AtomicUsize::new(0)),
            compact_active: Arc::new(AtomicUsize::new(0)),
            websocket_active: Arc::new(AtomicUsize::new(0)),
            standard_active: Arc::new(AtomicUsize::new(0)),
            responses_admissions_total: Arc::new(AtomicU64::new(0)),
            compact_admissions_total: Arc::new(AtomicU64::new(0)),
            websocket_admissions_total: Arc::new(AtomicU64::new(0)),
            standard_admissions_total: Arc::new(AtomicU64::new(0)),
            responses_releases_total: Arc::new(AtomicU64::new(0)),
            compact_releases_total: Arc::new(AtomicU64::new(0)),
            websocket_releases_total: Arc::new(AtomicU64::new(0)),
            standard_releases_total: Arc::new(AtomicU64::new(0)),
            responses_global_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            compact_global_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            websocket_global_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            standard_global_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            responses_lane_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            compact_lane_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            websocket_lane_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            standard_lane_limit_rejections_total: Arc::new(AtomicU64::new(0)),
            active_request_release_underflows_total: Arc::new(AtomicU64::new(0)),
            responses_release_underflows_total: Arc::new(AtomicU64::new(0)),
            compact_release_underflows_total: Arc::new(AtomicU64::new(0)),
            websocket_release_underflows_total: Arc::new(AtomicU64::new(0)),
            standard_release_underflows_total: Arc::new(AtomicU64::new(0)),
            profile_inflight_admissions_total: Arc::new(AtomicU64::new(0)),
            profile_inflight_releases_total: Arc::new(AtomicU64::new(0)),
            profile_inflight_release_underflows_total: Arc::new(AtomicU64::new(0)),
            profile_inflight: Arc::new(Mutex::new(BTreeMap::new())),
            admission_wait_metrics: Arc::new(RuntimeWaitDurationMetricCounters::default()),
            long_lived_queue_wait_metrics: Arc::new(RuntimeWaitDurationMetricCounters::default()),
            wait: Arc::new((Mutex::new(()), Condvar::new())),
            inflight_release_revision: Arc::new(AtomicU64::new(0)),
            limits,
        }
    }

    pub fn try_acquire(
        &self,
        active_request_count: Arc<AtomicUsize>,
        active_request_limit: usize,
        lane: RuntimeRouteKind,
        bypass_lane_limit: bool,
    ) -> Result<RuntimeProxyAdmissionAcquired, RuntimeProxyAdmissionLimit> {
        let lane_active_count = self.active_counter(lane);
        let lane_limit = self.limit(lane);
        loop {
            let active = active_request_count.load(Ordering::SeqCst);
            if active >= active_request_limit {
                return Err(RuntimeProxyAdmissionLimit::Global {
                    active,
                    limit: active_request_limit,
                });
            }
            let lane_active = lane_active_count.load(Ordering::SeqCst);
            if lane_active >= lane_limit && !bypass_lane_limit {
                return Err(RuntimeProxyAdmissionLimit::Lane {
                    active: lane_active,
                    limit: lane_limit,
                });
            }
            if active_request_count
                .compare_exchange(
                    active,
                    active.saturating_add(1),
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_err()
            {
                continue;
            }
            if lane_active_count
                .compare_exchange(
                    lane_active,
                    lane_active.saturating_add(1),
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_err()
            {
                active_request_count.fetch_sub(1, Ordering::SeqCst);
                continue;
            }
            self.admissions_total_counter(lane)
                .fetch_add(1, Ordering::Relaxed);
            return Ok(RuntimeProxyAdmissionAcquired {
                permit: RuntimeProxyAdmissionPermit {
                    active_request_count,
                    lane_active_count,
                    lane_releases_total: self.releases_total_counter(lane),
                    active_request_release_underflows_total: Arc::clone(
                        &self.active_request_release_underflows_total,
                    ),
                    lane_release_underflows_total: self.release_underflows_total_counter(lane),
                    wait: Arc::clone(&self.wait),
                },
                bypassed_lane_limit: lane_active >= lane_limit && bypass_lane_limit,
            });
        }
    }

    pub fn limits(&self) -> RuntimeProxyLaneLimits {
        self.limits
    }

    pub fn wait(&self) -> &(Mutex<()>, Condvar) {
        self.wait.as_ref()
    }

    pub fn admission_wait_metric_counters(&self) -> &RuntimeWaitDurationMetricCounters {
        self.admission_wait_metrics.as_ref()
    }

    pub fn long_lived_queue_wait_metric_counters(&self) -> &RuntimeWaitDurationMetricCounters {
        self.long_lived_queue_wait_metrics.as_ref()
    }

    pub fn active_request_release_underflows_total(&self) -> u64 {
        self.active_request_release_underflows_total
            .load(Ordering::Relaxed)
    }

    pub fn profile_inflight_count(&self, profile_name: &str) -> usize {
        self.profile_inflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(profile_name)
            .copied()
            .unwrap_or(0)
    }

    pub fn profile_inflight_snapshot(&self) -> BTreeMap<String, usize> {
        self.profile_inflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn acquire_profile_inflight(&self, profile_name: &str, weight: usize) -> usize {
        let mut inflight = self
            .profile_inflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let count = inflight.entry(profile_name.to_string()).or_default();
        *count = count.saturating_add(weight.max(1));
        self.profile_inflight_admissions_total
            .fetch_add(1, Ordering::Relaxed);
        *count
    }

    pub fn release_profile_inflight(
        &self,
        profile_name: &str,
        weight: usize,
    ) -> (usize, usize, bool) {
        let mut inflight = self
            .profile_inflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let weight = weight.max(1);
        let (remaining, count_before, underflow) = match inflight.get_mut(profile_name) {
            Some(count) if *count >= weight => {
                let count_before = *count;
                *count -= weight;
                (*count, count_before, false)
            }
            Some(count) => {
                let count_before = *count;
                *count = 0;
                (0, count_before, true)
            }
            None => (0, 0, true),
        };
        if remaining == 0 {
            inflight.remove(profile_name);
        }
        drop(inflight);
        self.profile_inflight_releases_total
            .fetch_add(1, Ordering::Relaxed);
        if underflow {
            self.profile_inflight_release_underflows_total
                .fetch_add(1, Ordering::Relaxed);
        }
        self.record_inflight_release();
        (remaining, count_before, underflow)
    }

    pub fn set_profile_inflight(&self, profile_name: impl Into<String>, count: usize) {
        let profile_name = profile_name.into();
        let mut inflight = self
            .profile_inflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if count == 0 {
            inflight.remove(&profile_name);
        } else {
            inflight.insert(profile_name, count);
        }
    }

    pub fn profile_inflight_admissions_total(&self) -> u64 {
        self.profile_inflight_admissions_total
            .load(Ordering::Relaxed)
    }

    pub fn profile_inflight_releases_total(&self) -> u64 {
        self.profile_inflight_releases_total.load(Ordering::Relaxed)
    }

    pub fn profile_inflight_release_underflows_total(&self) -> u64 {
        self.profile_inflight_release_underflows_total
            .load(Ordering::Relaxed)
    }

    pub fn inflight_release_revision(&self) -> u64 {
        self.inflight_release_revision.load(Ordering::SeqCst)
    }

    pub fn record_inflight_release(&self) {
        self.inflight_release_revision
            .fetch_add(1, Ordering::SeqCst);
        let (mutex, condvar) = self.wait();
        let _guard = mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        condvar.notify_all();
    }

    pub fn active_counter(&self, lane: RuntimeRouteKind) -> Arc<AtomicUsize> {
        match lane {
            RuntimeRouteKind::Responses => Arc::clone(&self.responses_active),
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_active),
            RuntimeRouteKind::Websocket => Arc::clone(&self.websocket_active),
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_active),
        }
    }

    pub fn limit(&self, lane: RuntimeRouteKind) -> usize {
        match lane {
            RuntimeRouteKind::Responses => self.limits.responses,
            RuntimeRouteKind::Compact => self.limits.compact,
            RuntimeRouteKind::Websocket => self.limits.websocket,
            RuntimeRouteKind::Standard => self.limits.standard,
        }
    }

    pub fn admissions_total_counter(&self, lane: RuntimeRouteKind) -> Arc<AtomicU64> {
        match lane {
            RuntimeRouteKind::Responses => Arc::clone(&self.responses_admissions_total),
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_admissions_total),
            RuntimeRouteKind::Websocket => Arc::clone(&self.websocket_admissions_total),
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_admissions_total),
        }
    }

    pub fn releases_total_counter(&self, lane: RuntimeRouteKind) -> Arc<AtomicU64> {
        match lane {
            RuntimeRouteKind::Responses => Arc::clone(&self.responses_releases_total),
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_releases_total),
            RuntimeRouteKind::Websocket => Arc::clone(&self.websocket_releases_total),
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_releases_total),
        }
    }

    pub fn global_limit_rejections_total_counter(&self, lane: RuntimeRouteKind) -> Arc<AtomicU64> {
        match lane {
            RuntimeRouteKind::Responses => {
                Arc::clone(&self.responses_global_limit_rejections_total)
            }
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_global_limit_rejections_total),
            RuntimeRouteKind::Websocket => {
                Arc::clone(&self.websocket_global_limit_rejections_total)
            }
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_global_limit_rejections_total),
        }
    }

    pub fn lane_limit_rejections_total_counter(&self, lane: RuntimeRouteKind) -> Arc<AtomicU64> {
        match lane {
            RuntimeRouteKind::Responses => Arc::clone(&self.responses_lane_limit_rejections_total),
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_lane_limit_rejections_total),
            RuntimeRouteKind::Websocket => Arc::clone(&self.websocket_lane_limit_rejections_total),
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_lane_limit_rejections_total),
        }
    }

    pub fn release_underflows_total_counter(&self, lane: RuntimeRouteKind) -> Arc<AtomicU64> {
        match lane {
            RuntimeRouteKind::Responses => Arc::clone(&self.responses_release_underflows_total),
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_release_underflows_total),
            RuntimeRouteKind::Websocket => Arc::clone(&self.websocket_release_underflows_total),
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_release_underflows_total),
        }
    }
}
