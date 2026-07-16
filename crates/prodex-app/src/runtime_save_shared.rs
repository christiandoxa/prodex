use crate::{
    AppPaths, AppState, ProfileEntry, RuntimeContinuationStore, RuntimeProfileBackoffs,
    RuntimeProfileHealth, RuntimeProfileUsageSnapshot, RuntimeRotationProxyShared,
    RuntimeSmartContextArtifactStore,
};
use std::path::PathBuf;
use std::time::Instant;

#[cfg(test)]
pub(crate) use prodex_runtime_state::{
    RuntimeDueJobs, RuntimeScheduledSaveJob, RuntimeStateSaveStateSection,
    runtime_take_due_scheduled_jobs,
};
pub(crate) use prodex_runtime_state::{
    RuntimeStateMutation, RuntimeStateSaveSections, runtime_run_scheduled_save_worker_loop,
};

pub(crate) type RuntimeStateSaveQueue =
    prodex_runtime_state::RuntimeStateSaveQueue<RuntimeStateSaveJob>;
pub(crate) type RuntimeContinuationJournalSaveQueue =
    prodex_runtime_state::RuntimeContinuationJournalSaveQueue<RuntimeContinuationJournalSaveJob>;

pub(crate) type RuntimeStateSaveSnapshot = prodex_runtime_state::RuntimeStateSaveSnapshot<
    AppPaths,
    AppState,
    RuntimeContinuationStore,
    RuntimeProfileHealth,
    RuntimeProfileUsageSnapshot,
    RuntimeProfileBackoffs,
>;

pub(crate) type RuntimeStateSaveSelectedSnapshot =
    prodex_runtime_state::RuntimeStateSaveSelectedSnapshot<
        AppPaths,
        AppState,
        ProfileEntry,
        RuntimeContinuationStore,
        RuntimeProfileHealth,
        RuntimeProfileUsageSnapshot,
        RuntimeProfileBackoffs,
    >;

pub(crate) type RuntimeStateSavePayload = prodex_runtime_state::RuntimeStateSavePayload<
    RuntimeStateSaveSnapshot,
    RuntimeRotationProxyShared,
>;
pub(crate) type RuntimeStateSaveJob =
    prodex_runtime_state::RuntimeStateSaveJob<RuntimeStateSavePayload>;

pub(crate) type RuntimeContinuationJournalSnapshot =
    prodex_runtime_state::RuntimeContinuationJournalSnapshot<
        AppPaths,
        RuntimeContinuationStore,
        ProfileEntry,
    >;
pub(crate) type RuntimeContinuationJournalSavePayload =
    prodex_runtime_state::RuntimeContinuationJournalSavePayload<
        RuntimeContinuationJournalSnapshot,
        RuntimeRotationProxyShared,
    >;
pub(crate) type RuntimeContinuationJournalSaveJob =
    prodex_runtime_state::RuntimeContinuationJournalSaveJob<RuntimeContinuationJournalSavePayload>;

pub(crate) type RuntimeProbeRefreshQueue =
    prodex_runtime_state::RuntimeProbeRefreshQueue<RuntimeProbeRefreshJob>;
pub(crate) type RuntimeProbeRefreshJob =
    prodex_runtime_state::RuntimeProbeRefreshJob<RuntimeRotationProxyShared>;

pub(crate) type RuntimeSmartContextArtifactSaveQueue =
    prodex_runtime_state::RuntimeStateSaveQueue<RuntimeSmartContextArtifactSaveJob>;

#[derive(Debug)]
pub(crate) struct RuntimeSmartContextArtifactSaveJob {
    pub(crate) path: PathBuf,
    pub(crate) store: RuntimeSmartContextArtifactStore,
    pub(crate) log_path: PathBuf,
    pub(crate) reason: String,
    pub(crate) queued_at: Instant,
    pub(crate) ready_at: Instant,
}

impl prodex_runtime_state::RuntimeScheduledSaveJob for RuntimeSmartContextArtifactSaveJob {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}
