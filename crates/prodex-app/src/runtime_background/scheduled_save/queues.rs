use std::collections::BTreeMap;
use std::sync::atomic::AtomicUsize;
#[cfg(test)]
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex};

use crate::{
    RUNTIME_CONTINUATION_JOURNAL_QUEUE_PRESSURE_THRESHOLD, RUNTIME_CONTINUATION_JOURNAL_SAVE_QUEUE,
    RUNTIME_PROBE_REFRESH_QUEUE_PRESSURE_THRESHOLD, RUNTIME_STATE_SAVE_QUEUE,
    RUNTIME_STATE_SAVE_QUEUE_PRESSURE_THRESHOLD,
};

use super::super::worker_spawn::spawn_runtime_background_worker_or_log;
use super::{
    RuntimeContinuationJournalSaveQueue, RuntimeStateSaveQueue,
    runtime_continuation_journal_save_worker_loop, runtime_state_save_worker_loop,
};

pub(crate) fn runtime_state_save_queue() -> Arc<RuntimeStateSaveQueue> {
    Arc::clone(RUNTIME_STATE_SAVE_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeStateSaveQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
            active: Arc::new(AtomicUsize::new(0)),
        });
        let worker_queue = Arc::clone(&queue);
        spawn_runtime_background_worker_or_log("prodex-runtime-state-save", None, move || {
            runtime_state_save_worker_loop(worker_queue)
        });
        queue
    }))
}

pub(crate) fn runtime_continuation_journal_save_queue() -> Arc<RuntimeContinuationJournalSaveQueue>
{
    Arc::clone(RUNTIME_CONTINUATION_JOURNAL_SAVE_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeContinuationJournalSaveQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
            active: Arc::new(AtomicUsize::new(0)),
        });
        let worker_queue = Arc::clone(&queue);
        spawn_runtime_background_worker_or_log(
            "prodex-runtime-continuation-journal-save",
            None,
            move || runtime_continuation_journal_save_worker_loop(worker_queue),
        );
        queue
    }))
}

pub(crate) fn runtime_state_save_queue_backlog() -> usize {
    runtime_state_save_queue()
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

#[cfg(test)]
pub(crate) fn runtime_state_save_queue_active() -> usize {
    runtime_state_save_queue().active.load(Ordering::SeqCst)
}

pub(crate) fn runtime_continuation_journal_queue_backlog() -> usize {
    runtime_continuation_journal_save_queue()
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

#[cfg(test)]
pub(crate) fn runtime_continuation_journal_queue_active() -> usize {
    runtime_continuation_journal_save_queue()
        .active
        .load(Ordering::SeqCst)
}

pub(crate) fn runtime_proxy_queue_pressure_active(
    state_save_backlog: usize,
    continuation_journal_backlog: usize,
    probe_refresh_backlog: usize,
) -> bool {
    prodex_runtime_state::runtime_proxy_queue_pressure_active(
        state_save_backlog,
        continuation_journal_backlog,
        probe_refresh_backlog,
        runtime_background_queue_pressure_thresholds(),
    )
}

pub(crate) fn runtime_background_queue_enqueue_plan(
    kind: prodex_runtime_state::RuntimeBackgroundQueueKind,
    pending_len_after_enqueue: usize,
) -> prodex_runtime_state::RuntimeBackgroundQueueEnqueuePlan {
    prodex_runtime_state::runtime_background_queue_enqueue_plan(
        kind,
        pending_len_after_enqueue,
        runtime_background_queue_pressure_thresholds(),
    )
}

fn runtime_background_queue_pressure_thresholds()
-> prodex_runtime_state::RuntimeBackgroundQueuePressureThresholds {
    prodex_runtime_state::RuntimeBackgroundQueuePressureThresholds {
        state_save: RUNTIME_STATE_SAVE_QUEUE_PRESSURE_THRESHOLD,
        continuation_journal: RUNTIME_CONTINUATION_JOURNAL_QUEUE_PRESSURE_THRESHOLD,
        probe_refresh: RUNTIME_PROBE_REFRESH_QUEUE_PRESSURE_THRESHOLD,
    }
}
