//! Runtime state and scheduled-save data structures.
//!
//! This crate intentionally owns side-effect-free state containers only. Live
//! proxy handles, transport clients, and persistence behavior stay in the
//! binary crate until their dependencies can be split safely.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    let value = Option::<T>::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeProfileBinding {
    pub profile_name: String,
    pub bound_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeQuotaWindowStatus {
    Ready,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(serialize = "B: Serialize", deserialize = "B: Deserialize<'de>"))]
pub struct RuntimeContinuationJournal<B = RuntimeProfileBinding> {
    #[serde(default)]
    pub saved_at: i64,
    #[serde(default)]
    pub continuations: RuntimeContinuationStore<B>,
}

impl<B> Default for RuntimeContinuationJournal<B> {
    fn default() -> Self {
        Self {
            saved_at: 0,
            continuations: RuntimeContinuationStore::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(serialize = "B: Serialize", deserialize = "B: Deserialize<'de>"))]
pub struct RuntimeContinuationStore<B = RuntimeProfileBinding> {
    #[serde(default)]
    pub response_profile_bindings: BTreeMap<String, B>,
    #[serde(default)]
    pub session_profile_bindings: BTreeMap<String, B>,
    #[serde(default)]
    pub turn_state_bindings: BTreeMap<String, B>,
    #[serde(default)]
    pub session_id_bindings: BTreeMap<String, B>,
    #[serde(default)]
    pub statuses: RuntimeContinuationStatuses,
}

impl<B> Default for RuntimeContinuationStore<B> {
    fn default() -> Self {
        Self {
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            statuses: RuntimeContinuationStatuses::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContinuationStatuses {
    #[serde(default)]
    pub response: BTreeMap<String, RuntimeContinuationBindingStatus>,
    #[serde(default)]
    pub turn_state: BTreeMap<String, RuntimeContinuationBindingStatus>,
    #[serde(default)]
    pub session_id: BTreeMap<String, RuntimeContinuationBindingStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RuntimeContinuationBindingLifecycle {
    #[default]
    Warm,
    Verified,
    Suspect,
    Dead,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContinuationBindingStatus {
    #[serde(default)]
    pub state: RuntimeContinuationBindingLifecycle,
    #[serde(default)]
    pub confidence: u32,
    #[serde(default)]
    pub last_touched_at: Option<i64>,
    #[serde(default)]
    pub last_verified_at: Option<i64>,
    #[serde(default)]
    pub last_verified_route: Option<String>,
    #[serde(default)]
    pub last_not_found_at: Option<i64>,
    #[serde(default)]
    pub not_found_streak: u32,
    #[serde(default)]
    pub success_count: u32,
    #[serde(default)]
    pub failure_count: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeProfileBackoffs {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub retry_backoff_until: BTreeMap<String, i64>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub transport_backoff_until: BTreeMap<String, i64>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub route_circuit_open_until: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeProfileUsageSnapshot<W = RuntimeQuotaWindowStatus> {
    pub checked_at: i64,
    pub five_hour_status: W,
    pub five_hour_remaining_percent: i64,
    pub five_hour_reset_at: i64,
    pub weekly_status: W,
    pub weekly_remaining_percent: i64,
    pub weekly_reset_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeProfileHealth {
    pub score: u32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProbeCacheFreshness {
    Fresh,
    StaleUsable,
    Expired,
}

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

#[derive(Debug)]
pub struct RuntimeStateSaveQueue<J> {
    pub pending: Mutex<BTreeMap<PathBuf, J>>,
    pub wake: Condvar,
    pub active: Arc<AtomicUsize>,
}

#[derive(Debug)]
pub struct RuntimeContinuationJournalSaveQueue<J> {
    pub pending: Mutex<BTreeMap<PathBuf, J>>,
    pub wake: Condvar,
    pub active: Arc<AtomicUsize>,
}

#[derive(Debug, Clone)]
pub struct RuntimeStateSaveSnapshot<P, S, C, H, U, B> {
    pub paths: P,
    pub state: S,
    pub continuations: C,
    pub profile_scores: BTreeMap<String, H>,
    pub usage_snapshots: BTreeMap<String, U>,
    pub backoffs: B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStateSaveStateSection {
    None,
    Core,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStateSaveSections {
    pub state: RuntimeStateSaveStateSection,
    pub continuations: bool,
    pub profile_scores: bool,
    pub usage_snapshots: bool,
    pub backoffs: bool,
}

impl RuntimeStateSaveSections {
    pub fn full() -> Self {
        Self {
            state: RuntimeStateSaveStateSection::Full,
            continuations: true,
            profile_scores: true,
            usage_snapshots: true,
            backoffs: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBackgroundQueuePressureThresholds {
    pub state_save: usize,
    pub continuation_journal: usize,
    pub probe_refresh: usize,
}

pub fn runtime_proxy_queue_pressure_active(
    state_save_backlog: usize,
    continuation_journal_backlog: usize,
    probe_refresh_backlog: usize,
    thresholds: RuntimeBackgroundQueuePressureThresholds,
) -> bool {
    state_save_backlog >= thresholds.state_save
        || continuation_journal_backlog >= thresholds.continuation_journal
        || probe_refresh_backlog >= thresholds.probe_refresh
}

pub fn runtime_background_enqueue_backlog(pending_len_after_enqueue: usize) -> usize {
    pending_len_after_enqueue.saturating_sub(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBackgroundQueueKind {
    StateSave,
    ContinuationJournal,
    ProbeRefresh,
}

impl RuntimeBackgroundQueuePressureThresholds {
    pub fn threshold_for(self, kind: RuntimeBackgroundQueueKind) -> usize {
        match kind {
            RuntimeBackgroundQueueKind::StateSave => self.state_save,
            RuntimeBackgroundQueueKind::ContinuationJournal => self.continuation_journal,
            RuntimeBackgroundQueueKind::ProbeRefresh => self.probe_refresh,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBackgroundQueueEnqueuePlan {
    pub backlog: usize,
    pub pressure_active: bool,
}

pub fn runtime_background_queue_enqueue_plan(
    kind: RuntimeBackgroundQueueKind,
    pending_len_after_enqueue: usize,
    thresholds: RuntimeBackgroundQueuePressureThresholds,
) -> RuntimeBackgroundQueueEnqueuePlan {
    let backlog = runtime_background_enqueue_backlog(pending_len_after_enqueue);
    RuntimeBackgroundQueueEnqueuePlan {
        backlog,
        pressure_active: backlog >= thresholds.threshold_for(kind),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStateSaveSchedulePlan {
    pub sections: RuntimeStateSaveSections,
    pub debounce: Duration,
    pub requires_continuation_journal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeScheduledSaveEnqueuePlan {
    pub queued_at: Instant,
    pub ready_at: Instant,
}

impl RuntimeScheduledSaveEnqueuePlan {
    pub fn ready_in(self) -> Duration {
        self.ready_at.saturating_duration_since(self.queued_at)
    }

    pub fn ready_in_ms(self) -> u128 {
        self.ready_in().as_millis()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStateSaveEnqueuePlan {
    pub schedule: RuntimeStateSaveSchedulePlan,
    pub queue: RuntimeScheduledSaveEnqueuePlan,
}

pub fn runtime_scheduled_save_enqueue_plan(
    queued_at: Instant,
    debounce: Duration,
) -> RuntimeScheduledSaveEnqueuePlan {
    RuntimeScheduledSaveEnqueuePlan {
        queued_at,
        ready_at: queued_at + debounce,
    }
}

pub fn runtime_state_save_schedule_plan(
    reason: &str,
    debounce: Duration,
) -> RuntimeStateSaveSchedulePlan {
    RuntimeStateSaveSchedulePlan {
        sections: runtime_state_save_sections_for_reason(reason),
        debounce: runtime_state_save_debounce(reason, debounce),
        requires_continuation_journal: runtime_state_save_reason_requires_continuation_journal(
            reason,
        ),
    }
}

pub fn runtime_state_save_enqueue_plan(
    reason: &str,
    queued_at: Instant,
    debounce: Duration,
) -> RuntimeStateSaveEnqueuePlan {
    let schedule = runtime_state_save_schedule_plan(reason, debounce);
    RuntimeStateSaveEnqueuePlan {
        schedule,
        queue: runtime_scheduled_save_enqueue_plan(queued_at, schedule.debounce),
    }
}

pub fn runtime_continuation_journal_save_enqueue_plan(
    reason: &str,
    queued_at: Instant,
    debounce: Duration,
) -> RuntimeScheduledSaveEnqueuePlan {
    runtime_scheduled_save_enqueue_plan(
        queued_at,
        runtime_continuation_journal_save_debounce(reason, debounce),
    )
}

pub fn runtime_state_save_reason_requires_continuation_journal(reason: &str) -> bool {
    [
        "response_ids:",
        "turn_state:",
        "session_id:",
        "compact_lineage:",
        "compact_lineage_release:",
    ]
    .into_iter()
    .any(|prefix| reason.starts_with(prefix))
}

pub fn runtime_state_save_sections_for_reason(reason: &str) -> RuntimeStateSaveSections {
    if matches!(reason, "startup_audit" | "startup_continuation_migration") {
        return RuntimeStateSaveSections::full();
    }

    let touches_continuations = [
        "response_ids:",
        "previous_response_owner:",
        "previous_response_negative_cache:",
        "previous_response_release:",
        "previous_response_binding_clear:",
        "response_touch:",
        "turn_state:",
        "turn_state_touch:",
        "session_id:",
        "session_touch:",
        "compact_lineage:",
        "compact_lineage_release:",
        "compact_session_touch:",
        "compact_turn_state_touch:",
        "dead_response_binding_clear:",
        "quota_release:",
        "continuation_stale:",
    ]
    .into_iter()
    .any(|prefix| reason.starts_with(prefix));
    if touches_continuations {
        let profile_scores = [
            "response_ids:",
            "previous_response_owner:",
            "previous_response_negative_cache:",
            "previous_response_release:",
        ]
        .into_iter()
        .any(|prefix| reason.starts_with(prefix));
        return RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::Core,
            continuations: true,
            profile_scores,
            usage_snapshots: false,
            backoffs: false,
        };
    }

    if reason.starts_with("profile_commit:") {
        return RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::Core,
            continuations: false,
            profile_scores: true,
            usage_snapshots: false,
            backoffs: true,
        };
    }

    if reason.starts_with("usage_snapshot:") || reason.starts_with("profile_retry_backoff:") {
        return RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::None,
            continuations: false,
            profile_scores: false,
            usage_snapshots: true,
            backoffs: true,
        };
    }

    if reason.starts_with("profile_transport_backoff:")
        || reason.starts_with("profile_circuit_half_open_probe:")
        || reason == "startup_backoff_soften"
    {
        return RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::None,
            continuations: false,
            profile_scores: false,
            usage_snapshots: false,
            backoffs: true,
        };
    }

    if reason.starts_with("profile_health:") || reason.starts_with("profile_circuit_clear:") {
        return RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::None,
            continuations: false,
            profile_scores: true,
            usage_snapshots: false,
            backoffs: true,
        };
    }

    if reason.starts_with("profile_bad_pairing:")
        || reason.starts_with("profile_auth_backoff:")
        || reason.starts_with("profile_auth_backoff_cleared:")
    {
        return RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::None,
            continuations: false,
            profile_scores: true,
            usage_snapshots: false,
            backoffs: false,
        };
    }

    RuntimeStateSaveSections::full()
}

pub fn runtime_hot_continuation_state_reason(reason: &str) -> bool {
    [
        "response_ids:",
        "previous_response_owner:",
        "response_touch:",
        "turn_state:",
        "turn_state_touch:",
        "session_id:",
        "session_touch:",
        "compact_lineage:",
        "compact_lineage_release:",
        "compact_session_touch:",
        "compact_turn_state_touch:",
    ]
    .into_iter()
    .any(|prefix| reason.starts_with(prefix))
}

pub fn runtime_state_save_debounce(reason: &str, debounce: Duration) -> Duration {
    if runtime_hot_continuation_state_reason(reason) {
        debounce
    } else {
        Duration::ZERO
    }
}

pub fn runtime_continuation_journal_save_debounce(reason: &str, debounce: Duration) -> Duration {
    if runtime_hot_continuation_state_reason(reason) {
        debounce
    } else {
        Duration::ZERO
    }
}

pub fn runtime_timestamp_touch_should_persist(
    timestamp: i64,
    now: i64,
    persist_interval_seconds: i64,
) -> bool {
    // Timestamps are persisted with second precision. Require strictly more
    // than interval so boundary crossings do not persist almost a second early.
    now.saturating_sub(timestamp) > persist_interval_seconds
}

pub fn runtime_probe_cache_freshness(
    checked_at: i64,
    now: i64,
    fresh_seconds: i64,
    stale_grace_seconds: i64,
) -> RuntimeProbeCacheFreshness {
    let age = now.saturating_sub(checked_at);
    if age <= fresh_seconds {
        RuntimeProbeCacheFreshness::Fresh
    } else if age <= stale_grace_seconds {
        RuntimeProbeCacheFreshness::StaleUsable
    } else {
        RuntimeProbeCacheFreshness::Expired
    }
}

pub fn runtime_profile_usage_snapshot_hold_active<W, F>(
    snapshot: &RuntimeProfileUsageSnapshot<W>,
    now: i64,
    is_exhausted: F,
) -> bool
where
    W: Copy,
    F: Fn(W) -> bool + Copy,
{
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| is_exhausted(status) && reset_at != i64::MAX && reset_at > now)
}

pub fn runtime_profile_usage_snapshot_hold_expired<W, F>(
    snapshot: &RuntimeProfileUsageSnapshot<W>,
    now: i64,
    is_exhausted: F,
) -> bool
where
    W: Copy,
    F: Fn(W) -> bool + Copy,
{
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| is_exhausted(status) && reset_at != i64::MAX && reset_at <= now)
}

pub fn runtime_profile_usage_snapshot_is_usable<W, F>(
    snapshot: &RuntimeProfileUsageSnapshot<W>,
    now: i64,
    stale_grace_seconds: i64,
    is_exhausted: F,
) -> bool
where
    W: Copy,
    F: Fn(W) -> bool + Copy,
{
    if runtime_profile_usage_snapshot_hold_active(snapshot, now, is_exhausted) {
        return true;
    }
    if runtime_profile_usage_snapshot_hold_expired(snapshot, now, is_exhausted) {
        return false;
    }
    now.saturating_sub(snapshot.checked_at) <= stale_grace_seconds
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStartupProbeRefreshCandidate<N> {
    pub profile_name: N,
    pub probe_fresh: bool,
    pub snapshot_usable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStartupProbeRefreshPlan {
    pub now: i64,
    pub probe_fresh_seconds: i64,
    pub stale_grace_seconds: i64,
    pub warm_limit: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeStartupProbeRefreshInput<'a, N, W> {
    pub profile_name: N,
    pub probe_checked_at: Option<i64>,
    pub usage_snapshot: Option<&'a RuntimeProfileUsageSnapshot<W>>,
}

pub fn runtime_profiles_needing_startup_probe_refresh<N, I>(
    candidates: I,
    warm_limit: usize,
) -> Vec<N>
where
    I: IntoIterator<Item = RuntimeStartupProbeRefreshCandidate<N>>,
{
    candidates
        .into_iter()
        .filter_map(|candidate| {
            (!candidate.probe_fresh && !candidate.snapshot_usable).then_some(candidate.profile_name)
        })
        .take(warm_limit)
        .collect()
}

pub fn runtime_profiles_needing_startup_probe_refresh_from_snapshots<'a, N, W, I, F>(
    inputs: I,
    plan: RuntimeStartupProbeRefreshPlan,
    is_exhausted: F,
) -> Vec<N>
where
    W: Copy + 'a,
    I: IntoIterator<Item = RuntimeStartupProbeRefreshInput<'a, N, W>>,
    F: Fn(W) -> bool + Copy,
{
    runtime_profiles_needing_startup_probe_refresh(
        inputs.into_iter().map(|input| {
            let probe_fresh = input.probe_checked_at.is_some_and(|checked_at| {
                runtime_probe_cache_freshness(
                    checked_at,
                    plan.now,
                    plan.probe_fresh_seconds,
                    plan.stale_grace_seconds,
                ) == RuntimeProbeCacheFreshness::Fresh
            });
            let snapshot_usable = input.usage_snapshot.is_some_and(|snapshot| {
                runtime_profile_usage_snapshot_is_usable(
                    snapshot,
                    plan.now,
                    plan.stale_grace_seconds,
                    is_exhausted,
                )
            });
            RuntimeStartupProbeRefreshCandidate {
                profile_name: input.profile_name,
                probe_fresh,
                snapshot_usable,
            }
        }),
        plan.warm_limit,
    )
}

pub fn runtime_profile_usage_snapshot_materially_matches<W: PartialEq>(
    previous: &RuntimeProfileUsageSnapshot<W>,
    next: &RuntimeProfileUsageSnapshot<W>,
) -> bool {
    previous.five_hour_status == next.five_hour_status
        && previous.five_hour_remaining_percent == next.five_hour_remaining_percent
        && previous.five_hour_reset_at == next.five_hour_reset_at
        && previous.weekly_status == next.weekly_status
        && previous.weekly_remaining_percent == next.weekly_remaining_percent
        && previous.weekly_reset_at == next.weekly_reset_at
}

pub fn runtime_profile_usage_snapshot_should_persist<W: PartialEq>(
    previous: Option<&RuntimeProfileUsageSnapshot<W>>,
    next: &RuntimeProfileUsageSnapshot<W>,
    now: i64,
    touch_persist_interval_seconds: i64,
) -> bool {
    let Some(previous) = previous else {
        return true;
    };

    !runtime_profile_usage_snapshot_materially_matches(previous, next)
        || runtime_timestamp_touch_should_persist(
            previous.checked_at,
            now,
            touch_persist_interval_seconds,
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProbeUsageSnapshotApplyPlan {
    pub snapshot_should_persist: bool,
    pub blocking_reset_at: Option<i64>,
    pub retry_backoff_until: Option<i64>,
    pub retry_backoff_changed: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeProbeUsageSnapshotApplyInput<'a, W> {
    pub previous_snapshot: Option<&'a RuntimeProfileUsageSnapshot<W>>,
    pub previous_retry_backoff_until: Option<i64>,
    pub next_snapshot: &'a RuntimeProfileUsageSnapshot<W>,
    pub quota_blocked: bool,
    pub blocking_reset_at: Option<i64>,
    pub now: i64,
    pub quota_quarantine_fallback_seconds: i64,
    pub touch_persist_interval_seconds: i64,
}

pub fn runtime_probe_usage_snapshot_apply_plan<W: PartialEq>(
    input: RuntimeProbeUsageSnapshotApplyInput<'_, W>,
) -> RuntimeProbeUsageSnapshotApplyPlan {
    let snapshot_should_persist = runtime_profile_usage_snapshot_should_persist(
        input.previous_snapshot,
        input.next_snapshot,
        input.now,
        input.touch_persist_interval_seconds,
    );
    let blocking_reset_at = input
        .blocking_reset_at
        .filter(|reset_at| *reset_at > input.now);
    let quarantine_until = input.quota_blocked.then(|| {
        blocking_reset_at.unwrap_or_else(|| {
            input
                .now
                .saturating_add(input.quota_quarantine_fallback_seconds)
        })
    });
    let retry_backoff_until = quarantine_until.map(|until| {
        input
            .previous_retry_backoff_until
            .unwrap_or(until)
            .max(until)
    });
    let retry_backoff_changed =
        retry_backoff_until.is_some_and(|until| Some(until) != input.previous_retry_backoff_until);

    RuntimeProbeUsageSnapshotApplyPlan {
        snapshot_should_persist,
        blocking_reset_at,
        retry_backoff_until,
        retry_backoff_changed,
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeStateSaveSelectedSnapshot<P, S, E, C, H, U, B> {
    pub paths: P,
    pub state: Option<S>,
    pub profiles: Option<BTreeMap<String, E>>,
    pub continuations: Option<C>,
    pub profile_scores: Option<BTreeMap<String, H>>,
    pub usage_snapshots: Option<BTreeMap<String, U>>,
    pub backoffs: Option<B>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum RuntimeStateSavePayload<S, Shared> {
    Snapshot(S),
    Live {
        shared: Shared,
        sections: RuntimeStateSaveSections,
    },
}

#[derive(Debug)]
pub struct RuntimeStateSaveJob<P> {
    pub payload: P,
    pub revision: u64,
    pub latest_revision: Arc<AtomicU64>,
    pub log_path: PathBuf,
    pub reason: String,
    pub queued_at: Instant,
    pub ready_at: Instant,
}

#[derive(Debug, Clone)]
pub struct RuntimeContinuationJournalSnapshot<P, C, E> {
    pub paths: P,
    pub continuations: C,
    pub profiles: BTreeMap<String, E>,
}

#[derive(Debug, Clone)]
pub enum RuntimeContinuationJournalSavePayload<S, Shared> {
    Snapshot(S),
    Live(Shared),
}

#[derive(Debug)]
pub struct RuntimeContinuationJournalSaveJob<P> {
    pub payload: P,
    pub log_path: PathBuf,
    pub reason: String,
    pub saved_at: i64,
    pub queued_at: Instant,
    pub ready_at: Instant,
}

pub trait RuntimeScheduledSaveJob {
    fn ready_at(&self) -> Instant;
}

pub fn runtime_state_snapshot_is_latest_revision(
    latest_revision: &AtomicU64,
    revision: u64,
) -> bool {
    latest_revision.load(Ordering::SeqCst) == revision
}

impl<P> RuntimeScheduledSaveJob for RuntimeStateSaveJob<P> {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

impl<P> RuntimeScheduledSaveJob for RuntimeContinuationJournalSaveJob<P> {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

pub enum RuntimeDueJobs<K, J> {
    Due(BTreeMap<K, J>),
    Wait(Duration),
}

pub fn runtime_take_due_scheduled_jobs<K, J>(
    pending: &mut BTreeMap<K, J>,
    now: Instant,
) -> RuntimeDueJobs<K, J>
where
    K: Ord + Clone,
    J: RuntimeScheduledSaveJob,
{
    if pending.is_empty() {
        return RuntimeDueJobs::Due(BTreeMap::new());
    }

    let next_ready_at = pending
        .values()
        .map(RuntimeScheduledSaveJob::ready_at)
        .min()
        .expect("pending scheduled save jobs should be non-empty after guard");
    if next_ready_at > now {
        return RuntimeDueJobs::Wait(next_ready_at.saturating_duration_since(now));
    }

    let due_keys = pending
        .iter()
        .filter_map(|(key, job)| (job.ready_at() <= now).then_some(key.clone()))
        .collect::<Vec<_>>();
    let mut due = BTreeMap::new();
    for key in due_keys {
        if let Some(job) = pending.remove(&key) {
            due.insert(key, job);
        }
    }
    RuntimeDueJobs::Due(due)
}

pub fn runtime_wait_for_due_scheduled_jobs<K, J>(
    pending: &Mutex<BTreeMap<K, J>>,
    wake: &Condvar,
) -> BTreeMap<K, J>
where
    K: Ord + Clone,
    J: RuntimeScheduledSaveJob,
{
    let mut pending = pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while pending.is_empty() {
        pending = wake
            .wait(pending)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
    loop {
        match runtime_take_due_scheduled_jobs(&mut pending, Instant::now()) {
            RuntimeDueJobs::Due(jobs) => break jobs,
            RuntimeDueJobs::Wait(wait_for) => {
                let (next_pending, _) = wake
                    .wait_timeout(pending, wait_for)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                pending = next_pending;
            }
        }
    }
}

pub fn runtime_run_scheduled_save_worker_loop<K, J, F>(
    pending: &Mutex<BTreeMap<K, J>>,
    wake: &Condvar,
    active: &AtomicUsize,
    mut run_job: F,
) -> !
where
    K: Ord + Clone,
    J: RuntimeScheduledSaveJob,
    F: FnMut(J),
{
    loop {
        let jobs = runtime_wait_for_due_scheduled_jobs(pending, wake);
        for (_, job) in jobs {
            active.fetch_add(1, Ordering::SeqCst);
            run_job(job);
            active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

#[derive(Debug)]
pub struct RuntimeProbeRefreshQueue<J> {
    pub pending: Mutex<BTreeMap<(PathBuf, String), J>>,
    pub wake: Condvar,
    pub active: Arc<AtomicUsize>,
    pub wait: Arc<(Mutex<()>, Condvar)>,
    pub revision: Arc<AtomicU64>,
}

#[derive(Debug, Clone)]
pub struct RuntimeProbeRefreshJob<Shared> {
    pub shared: Shared,
    pub profile_name: String,
    pub codex_home: PathBuf,
    pub upstream_base_url: String,
    pub queued_at: Instant,
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
