//! Runtime state and scheduled-save data structures.
//!
//! This crate intentionally owns side-effect-free state containers only. Live
//! proxy handles, transport clients, and persistence behavior stay in the
//! binary crate until their dependencies can be split safely.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize};
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
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestJob {
        ready_at: Instant,
    }

    impl RuntimeScheduledSaveJob for TestJob {
        fn ready_at(&self) -> Instant {
            self.ready_at
        }
    }

    #[test]
    fn due_jobs_are_removed_without_disturbing_future_jobs() {
        let now = Instant::now();
        let mut pending = BTreeMap::from([
            ("due-a", TestJob { ready_at: now }),
            (
                "future",
                TestJob {
                    ready_at: now + Duration::from_secs(5),
                },
            ),
            (
                "due-b",
                TestJob {
                    ready_at: now - Duration::from_secs(1),
                },
            ),
        ]);

        match runtime_take_due_scheduled_jobs(&mut pending, now) {
            RuntimeDueJobs::Due(due) => {
                assert_eq!(due.len(), 2);
                assert!(due.contains_key("due-a"));
                assert!(due.contains_key("due-b"));
            }
            RuntimeDueJobs::Wait(_) => panic!("expected due jobs"),
        }

        assert_eq!(pending.len(), 1);
        assert!(pending.contains_key("future"));
    }

    #[test]
    fn future_jobs_return_wait_duration() {
        let now = Instant::now();
        let mut pending = BTreeMap::from([(
            "future",
            TestJob {
                ready_at: now + Duration::from_secs(3),
            },
        )]);

        match runtime_take_due_scheduled_jobs(&mut pending, now) {
            RuntimeDueJobs::Due(_) => panic!("expected wait"),
            RuntimeDueJobs::Wait(wait) => assert_eq!(wait, Duration::from_secs(3)),
        }

        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn save_sections_follow_dirty_reason_scope() {
        assert_eq!(
            runtime_state_save_sections_for_reason("usage_snapshot:main"),
            RuntimeStateSaveSections {
                state: RuntimeStateSaveStateSection::None,
                continuations: false,
                profile_scores: false,
                usage_snapshots: true,
                backoffs: true,
            }
        );
        assert_eq!(
            runtime_state_save_sections_for_reason("response_ids:main"),
            RuntimeStateSaveSections {
                state: RuntimeStateSaveStateSection::Core,
                continuations: true,
                profile_scores: true,
                usage_snapshots: false,
                backoffs: false,
            }
        );
        assert_eq!(
            runtime_state_save_sections_for_reason("profile_commit:second"),
            RuntimeStateSaveSections {
                state: RuntimeStateSaveStateSection::Core,
                continuations: false,
                profile_scores: true,
                usage_snapshots: false,
                backoffs: true,
            }
        );
        assert_eq!(
            runtime_state_save_sections_for_reason("startup_audit"),
            RuntimeStateSaveSections::full()
        );
    }

    #[test]
    fn debounce_only_applies_to_hot_continuation_reasons() {
        let debounce = Duration::from_millis(150);

        assert_eq!(
            runtime_state_save_debounce("turn_state:abc", debounce),
            debounce
        );
        assert_eq!(
            runtime_continuation_journal_save_debounce("profile_commit:main", debounce),
            Duration::ZERO
        );
    }

    #[test]
    fn pressure_helper_checks_each_queue_threshold() {
        let thresholds = RuntimeBackgroundQueuePressureThresholds {
            state_save: 8,
            continuation_journal: 8,
            probe_refresh: 16,
        };

        assert!(runtime_proxy_queue_pressure_active(8, 0, 0, thresholds));
        assert!(runtime_proxy_queue_pressure_active(0, 8, 0, thresholds));
        assert!(runtime_proxy_queue_pressure_active(0, 0, 16, thresholds));
        assert!(!runtime_proxy_queue_pressure_active(7, 7, 15, thresholds));
    }
}
