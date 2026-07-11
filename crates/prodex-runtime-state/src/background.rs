use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

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

    let Some(next_ready_at) = pending
        .values()
        .map(RuntimeScheduledSaveJob::ready_at)
        .min()
    else {
        return RuntimeDueJobs::Due(BTreeMap::new());
    };
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
