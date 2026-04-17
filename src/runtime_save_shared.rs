use crate::{
    AppPaths, AppState, ProfileEntry, RuntimeContinuationStore, RuntimeProfileBackoffs,
    RuntimeProfileHealth, RuntimeProfileUsageSnapshot, RuntimeRotationProxyShared,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) struct RuntimeStateSaveQueue {
    pub(crate) pending: Mutex<BTreeMap<PathBuf, RuntimeStateSaveJob>>,
    pub(crate) wake: Condvar,
    pub(crate) active: Arc<AtomicUsize>,
}

#[derive(Debug)]
pub(crate) struct RuntimeContinuationJournalSaveQueue {
    pub(crate) pending: Mutex<BTreeMap<PathBuf, RuntimeContinuationJournalSaveJob>>,
    pub(crate) wake: Condvar,
    pub(crate) active: Arc<AtomicUsize>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeStateSaveSnapshot {
    pub(crate) paths: AppPaths,
    pub(crate) state: AppState,
    pub(crate) continuations: RuntimeContinuationStore,
    pub(crate) profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    pub(crate) usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    pub(crate) backoffs: RuntimeProfileBackoffs,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum RuntimeStateSavePayload {
    Snapshot(RuntimeStateSaveSnapshot),
    Live(RuntimeRotationProxyShared),
}

#[derive(Debug)]
pub(crate) struct RuntimeStateSaveJob {
    pub(crate) payload: RuntimeStateSavePayload,
    pub(crate) revision: u64,
    pub(crate) latest_revision: Arc<AtomicU64>,
    pub(crate) log_path: PathBuf,
    pub(crate) reason: String,
    pub(crate) queued_at: Instant,
    pub(crate) ready_at: Instant,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeContinuationJournalSnapshot {
    pub(crate) paths: AppPaths,
    pub(crate) continuations: RuntimeContinuationStore,
    pub(crate) profiles: BTreeMap<String, ProfileEntry>,
}

#[derive(Debug, Clone)]
pub(crate) enum RuntimeContinuationJournalSavePayload {
    Snapshot(RuntimeContinuationJournalSnapshot),
    Live(RuntimeRotationProxyShared),
}

#[derive(Debug)]
pub(crate) struct RuntimeContinuationJournalSaveJob {
    pub(crate) payload: RuntimeContinuationJournalSavePayload,
    pub(crate) log_path: PathBuf,
    pub(crate) reason: String,
    pub(crate) saved_at: i64,
    pub(crate) queued_at: Instant,
    pub(crate) ready_at: Instant,
}

pub(crate) trait RuntimeScheduledSaveJob {
    fn ready_at(&self) -> Instant;
}

impl RuntimeScheduledSaveJob for RuntimeStateSaveJob {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

impl RuntimeScheduledSaveJob for RuntimeContinuationJournalSaveJob {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

pub(crate) enum RuntimeDueJobs<K, J> {
    Due(BTreeMap<K, J>),
    Wait(Duration),
}

pub(crate) fn runtime_take_due_scheduled_jobs<K, J>(
    pending: &mut BTreeMap<K, J>,
    now: Instant,
) -> RuntimeDueJobs<K, J>
where
    K: Ord + Clone,
    J: RuntimeScheduledSaveJob,
{
    let next_ready_at = pending
        .values()
        .map(RuntimeScheduledSaveJob::ready_at)
        .min()
        .expect("scheduled save jobs should be present");
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
pub(crate) struct RuntimeProbeRefreshQueue {
    pub(crate) pending: Mutex<BTreeMap<(PathBuf, String), RuntimeProbeRefreshJob>>,
    pub(crate) wake: Condvar,
    pub(crate) active: Arc<AtomicUsize>,
    pub(crate) wait: Arc<(Mutex<()>, Condvar)>,
    pub(crate) revision: Arc<AtomicU64>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeProbeRefreshJob {
    pub(crate) shared: RuntimeRotationProxyShared,
    pub(crate) profile_name: String,
    pub(crate) codex_home: PathBuf,
    pub(crate) upstream_base_url: String,
    pub(crate) queued_at: Instant,
}
