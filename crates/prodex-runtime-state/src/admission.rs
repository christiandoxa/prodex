use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeRouteKind {
    Responses,
    Compact,
    Websocket,
    Standard,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeStateLockWaitMetrics {
    pub wait_total_ns: u64,
    pub wait_count: u64,
    pub wait_max_ns: u64,
}

#[derive(Debug, Default)]
pub struct RuntimeStateLockWaitMetricCounters {
    wait_total_ns: AtomicU64,
    wait_count: AtomicU64,
    wait_max_ns: AtomicU64,
}

impl RuntimeStateLockWaitMetricCounters {
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

    pub fn snapshot(&self) -> RuntimeStateLockWaitMetrics {
        RuntimeStateLockWaitMetrics {
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

#[derive(Debug, Clone, Copy)]
pub struct RuntimeProxyLaneLimits {
    pub responses: usize,
    pub compact: usize,
    pub websocket: usize,
    pub standard: usize,
}

#[derive(Debug, Clone)]
pub struct RuntimeProxyLaneAdmission {
    pub responses_active: Arc<AtomicUsize>,
    pub compact_active: Arc<AtomicUsize>,
    pub websocket_active: Arc<AtomicUsize>,
    pub standard_active: Arc<AtomicUsize>,
    pub responses_admissions_total: Arc<AtomicU64>,
    pub compact_admissions_total: Arc<AtomicU64>,
    pub websocket_admissions_total: Arc<AtomicU64>,
    pub standard_admissions_total: Arc<AtomicU64>,
    pub responses_releases_total: Arc<AtomicU64>,
    pub compact_releases_total: Arc<AtomicU64>,
    pub websocket_releases_total: Arc<AtomicU64>,
    pub standard_releases_total: Arc<AtomicU64>,
    pub responses_global_limit_rejections_total: Arc<AtomicU64>,
    pub compact_global_limit_rejections_total: Arc<AtomicU64>,
    pub websocket_global_limit_rejections_total: Arc<AtomicU64>,
    pub standard_global_limit_rejections_total: Arc<AtomicU64>,
    pub responses_lane_limit_rejections_total: Arc<AtomicU64>,
    pub compact_lane_limit_rejections_total: Arc<AtomicU64>,
    pub websocket_lane_limit_rejections_total: Arc<AtomicU64>,
    pub standard_lane_limit_rejections_total: Arc<AtomicU64>,
    pub active_request_release_underflows_total: Arc<AtomicU64>,
    pub responses_release_underflows_total: Arc<AtomicU64>,
    pub compact_release_underflows_total: Arc<AtomicU64>,
    pub websocket_release_underflows_total: Arc<AtomicU64>,
    pub standard_release_underflows_total: Arc<AtomicU64>,
    pub profile_inflight_admissions_total: Arc<AtomicU64>,
    pub profile_inflight_releases_total: Arc<AtomicU64>,
    pub profile_inflight_release_underflows_total: Arc<AtomicU64>,
    pub wait: Arc<(Mutex<()>, Condvar)>,
    pub inflight_release_revision: Arc<AtomicU64>,
    pub limits: RuntimeProxyLaneLimits,
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
            wait: Arc::new((Mutex::new(()), Condvar::new())),
            inflight_release_revision: Arc::new(AtomicU64::new(0)),
            limits,
        }
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
