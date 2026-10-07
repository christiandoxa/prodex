use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeStateMutation {
    FullState,
    StartupAudit,
    StartupContinuationMigration,
    StartupBackoffSoften,
    ResponseIds(String),
    PreviousResponseOwner(String),
    PreviousResponseNegativeCache(String),
    PreviousResponseRelease(String),
    ResponseTouch(String),
    TurnState(String),
    TurnStateTouch(String),
    SessionId(String),
    SessionTouch(String),
    SessionAffinityRelease(String),
    CompactLineage(String),
    CompactLineageRelease(String),
    CompactSessionTouch(String),
    CompactTurnStateTouch(String),
    DeadResponseBindingClear(String),
    QuotaRelease(String),
    AuthFailedRelease(String),
    ContinuationStale(String),
    ProfileCommit(String),
    UsageSnapshot(String),
    ProfileRetryBackoff(String),
    ProfileTransportBackoff(String),
    ProfileCircuitHalfOpenProbe(String),
    ProfileHealth(String),
    ProfileCircuitClear(String),
    ProfileBadPairing(String),
    ProfileAuthBackoff(String),
    ProfileAuthBackoffCleared(String),
}

fn runtime_state_mutation_input(mutation: &RuntimeStateMutation) -> (u8, Option<&str>) {
    match mutation {
        RuntimeStateMutation::FullState => (0, None),
        RuntimeStateMutation::StartupAudit => (1, None),
        RuntimeStateMutation::StartupContinuationMigration => (2, None),
        RuntimeStateMutation::StartupBackoffSoften => (3, None),
        RuntimeStateMutation::ResponseIds(value) => (4, Some(value)),
        RuntimeStateMutation::PreviousResponseOwner(value) => (5, Some(value)),
        RuntimeStateMutation::PreviousResponseNegativeCache(value) => (6, Some(value)),
        RuntimeStateMutation::PreviousResponseRelease(value) => (7, Some(value)),
        RuntimeStateMutation::ResponseTouch(value) => (8, Some(value)),
        RuntimeStateMutation::TurnState(value) => (9, Some(value)),
        RuntimeStateMutation::TurnStateTouch(value) => (10, Some(value)),
        RuntimeStateMutation::SessionId(value) => (11, Some(value)),
        RuntimeStateMutation::SessionTouch(value) => (12, Some(value)),
        RuntimeStateMutation::SessionAffinityRelease(value) => (13, Some(value)),
        RuntimeStateMutation::CompactLineage(value) => (14, Some(value)),
        RuntimeStateMutation::CompactLineageRelease(value) => (15, Some(value)),
        RuntimeStateMutation::CompactSessionTouch(value) => (16, Some(value)),
        RuntimeStateMutation::CompactTurnStateTouch(value) => (17, Some(value)),
        RuntimeStateMutation::DeadResponseBindingClear(value) => (18, Some(value)),
        RuntimeStateMutation::QuotaRelease(value) => (19, Some(value)),
        RuntimeStateMutation::AuthFailedRelease(value) => (20, Some(value)),
        RuntimeStateMutation::ContinuationStale(value) => (21, Some(value)),
        RuntimeStateMutation::ProfileCommit(value) => (22, Some(value)),
        RuntimeStateMutation::UsageSnapshot(value) => (23, Some(value)),
        RuntimeStateMutation::ProfileRetryBackoff(value) => (24, Some(value)),
        RuntimeStateMutation::ProfileTransportBackoff(value) => (25, Some(value)),
        RuntimeStateMutation::ProfileCircuitHalfOpenProbe(value) => (26, Some(value)),
        RuntimeStateMutation::ProfileHealth(value) => (27, Some(value)),
        RuntimeStateMutation::ProfileCircuitClear(value) => (28, Some(value)),
        RuntimeStateMutation::ProfileBadPairing(value) => (29, Some(value)),
        RuntimeStateMutation::ProfileAuthBackoff(value) => (30, Some(value)),
        RuntimeStateMutation::ProfileAuthBackoffCleared(value) => (31, Some(value)),
    }
}

impl RuntimeStateMutation {
    pub fn reason(&self) -> String {
        let (kind, value) = runtime_state_mutation_input(self);
        prodex_mojo_core::runtime_state::mutation_reason(kind, value)
            .expect("Mojo runtime-state mutation reason returned invalid output")
    }
}

fn runtime_state_mutation_policy(
    mutation: &RuntimeStateMutation,
) -> prodex_mojo_core::runtime_state::RuntimeStateMutationPolicy {
    prodex_mojo_core::runtime_state::mutation_policy(runtime_state_mutation_input(mutation).0)
        .expect("Mojo runtime-state mutation policy returned invalid output")
}

fn runtime_state_sections_from_policy(
    policy: prodex_mojo_core::runtime_state::RuntimeStateMutationPolicy,
) -> RuntimeStateSaveSections {
    RuntimeStateSaveSections {
        state: match policy.state_section {
            0 => RuntimeStateSaveStateSection::None,
            1 => RuntimeStateSaveStateSection::Core,
            2 => RuntimeStateSaveStateSection::Full,
            _ => unreachable!("validated Mojo runtime-state section tag"),
        },
        continuations: policy.continuations,
        profile_scores: policy.profile_scores,
        usage_snapshots: policy.usage_snapshots,
        backoffs: policy.backoffs,
    }
}

fn runtime_background_thresholds(
    thresholds: RuntimeBackgroundQueuePressureThresholds,
) -> [usize; 3] {
    [
        thresholds.state_save,
        thresholds.continuation_journal,
        thresholds.probe_refresh,
    ]
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

    pub fn union(self, other: Self) -> Self {
        let state =
            match (self.state, other.state) {
                (RuntimeStateSaveStateSection::Full, _)
                | (_, RuntimeStateSaveStateSection::Full) => RuntimeStateSaveStateSection::Full,
                (RuntimeStateSaveStateSection::Core, _)
                | (_, RuntimeStateSaveStateSection::Core) => RuntimeStateSaveStateSection::Core,
                _ => RuntimeStateSaveStateSection::None,
            };
        Self {
            state,
            continuations: self.continuations || other.continuations,
            profile_scores: self.profile_scores || other.profile_scores,
            usage_snapshots: self.usage_snapshots || other.usage_snapshots,
            backoffs: self.backoffs || other.backoffs,
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
    prodex_mojo_core::runtime_state::queue_pressure_active(
        [
            state_save_backlog,
            continuation_journal_backlog,
            probe_refresh_backlog,
        ],
        runtime_background_thresholds(thresholds),
    )
    .expect("Mojo runtime background queue-pressure policy returned invalid output")
}

pub fn runtime_background_enqueue_backlog(pending_len_after_enqueue: usize) -> usize {
    prodex_mojo_core::runtime_state::enqueue_backlog(pending_len_after_enqueue)
        .expect("Mojo runtime background backlog policy returned invalid output")
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBackgroundQueueKind {
    StateSave = 0,
    ContinuationJournal = 1,
    ProbeRefresh = 2,
}

impl RuntimeBackgroundQueuePressureThresholds {
    pub fn threshold_for(self, kind: RuntimeBackgroundQueueKind) -> usize {
        prodex_mojo_core::runtime_state::queue_threshold(
            kind as u8,
            runtime_background_thresholds(self),
        )
        .expect("Mojo runtime background threshold policy returned invalid output")
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
    let plan = prodex_mojo_core::runtime_state::queue_enqueue_plan(
        kind as u8,
        pending_len_after_enqueue,
        runtime_background_thresholds(thresholds),
    )
    .expect("Mojo runtime background enqueue policy returned invalid output");
    RuntimeBackgroundQueueEnqueuePlan {
        backlog: plan.backlog,
        pressure_active: plan.pressure_active,
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
    mutation: &RuntimeStateMutation,
    debounce: Duration,
) -> RuntimeStateSaveSchedulePlan {
    let policy = runtime_state_mutation_policy(mutation);
    RuntimeStateSaveSchedulePlan {
        sections: runtime_state_sections_from_policy(policy),
        debounce: if policy.hot_continuation_state {
            debounce
        } else {
            Duration::ZERO
        },
        requires_continuation_journal: policy.requires_continuation_journal,
    }
}

pub fn runtime_state_save_enqueue_plan(
    mutation: &RuntimeStateMutation,
    queued_at: Instant,
    debounce: Duration,
) -> RuntimeStateSaveEnqueuePlan {
    let schedule = runtime_state_save_schedule_plan(mutation, debounce);
    RuntimeStateSaveEnqueuePlan {
        schedule,
        queue: runtime_scheduled_save_enqueue_plan(queued_at, schedule.debounce),
    }
}

pub fn runtime_continuation_journal_save_enqueue_plan(
    mutation: &RuntimeStateMutation,
    queued_at: Instant,
    debounce: Duration,
) -> RuntimeScheduledSaveEnqueuePlan {
    runtime_scheduled_save_enqueue_plan(
        queued_at,
        runtime_continuation_journal_save_debounce(mutation, debounce),
    )
}

pub fn runtime_state_save_requires_continuation_journal(mutation: &RuntimeStateMutation) -> bool {
    runtime_state_mutation_policy(mutation).requires_continuation_journal
}

pub fn runtime_state_save_sections(mutation: &RuntimeStateMutation) -> RuntimeStateSaveSections {
    runtime_state_sections_from_policy(runtime_state_mutation_policy(mutation))
}

pub fn runtime_hot_continuation_state_mutation(mutation: &RuntimeStateMutation) -> bool {
    runtime_state_mutation_policy(mutation).hot_continuation_state
}

pub fn runtime_state_save_debounce(
    mutation: &RuntimeStateMutation,
    debounce: Duration,
) -> Duration {
    if runtime_hot_continuation_state_mutation(mutation) {
        debounce
    } else {
        Duration::ZERO
    }
}

pub fn runtime_continuation_journal_save_debounce(
    mutation: &RuntimeStateMutation,
    debounce: Duration,
) -> Duration {
    if runtime_hot_continuation_state_mutation(mutation) {
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
    pub scheduled: Mutex<BTreeSet<(PathBuf, String)>>,
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
