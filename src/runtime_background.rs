use super::*;

pub(super) fn runtime_persistence_mode_by_log_path() -> &'static Mutex<BTreeMap<PathBuf, bool>> {
    RUNTIME_PERSISTENCE_MODE_BY_LOG_PATH.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub(super) fn register_runtime_proxy_persistence_mode(log_path: &Path, enabled: bool) {
    let mut modes = runtime_persistence_mode_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    modes.insert(log_path.to_path_buf(), enabled);
}

pub(super) fn unregister_runtime_proxy_persistence_mode(log_path: &Path) {
    let mut modes = runtime_persistence_mode_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    modes.remove(log_path);
}

pub(super) fn runtime_proxy_persistence_enabled_for_log_path(log_path: &Path) -> bool {
    runtime_persistence_mode_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(log_path)
        .copied()
        .unwrap_or(true)
}

pub(super) fn runtime_proxy_persistence_enabled(shared: &RuntimeRotationProxyShared) -> bool {
    runtime_proxy_persistence_enabled_for_log_path(&shared.log_path)
}

pub(super) fn runtime_broker_metadata_by_log_path()
-> &'static Mutex<BTreeMap<PathBuf, RuntimeBrokerMetadata>> {
    RUNTIME_BROKER_METADATA_BY_LOG_PATH.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub(super) fn register_runtime_broker_metadata(log_path: &Path, metadata: RuntimeBrokerMetadata) {
    let mut metadata_by_path = runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    metadata_by_path.insert(log_path.to_path_buf(), metadata);
}

pub(super) fn unregister_runtime_broker_metadata(log_path: &Path) {
    let mut metadata_by_path = runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    metadata_by_path.remove(log_path);
}

#[allow(dead_code)]
pub(super) fn runtime_broker_metadata_for_log_path(
    log_path: &Path,
) -> Option<RuntimeBrokerMetadata> {
    runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(log_path)
        .cloned()
}

pub(super) struct RuntimeStateSaveRequest {
    state: AppState,
    continuations: RuntimeContinuationStore,
    profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: RuntimeProfileBackoffs,
    paths: AppPaths,
    reason: String,
}

impl RuntimeStateSaveRequest {
    fn from_snapshot(snapshot: RuntimeStateSaveSnapshot, reason: &str) -> Self {
        Self {
            state: snapshot.state,
            continuations: snapshot.continuations,
            profile_scores: snapshot.profile_scores,
            usage_snapshots: snapshot.usage_snapshots,
            backoffs: snapshot.backoffs,
            paths: snapshot.paths,
            reason: reason.to_string(),
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
pub(super) fn schedule_runtime_state_save(
    shared: &RuntimeRotationProxyShared,
    state: AppState,
    continuations: RuntimeContinuationStore,
    profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: RuntimeProfileBackoffs,
    paths: AppPaths,
    reason: &str,
) {
    schedule_runtime_state_save_request(
        shared,
        RuntimeStateSaveRequest {
            state,
            continuations,
            profile_scores,
            usage_snapshots,
            backoffs,
            paths,
            reason: reason.to_string(),
        },
    );
}

pub(super) fn schedule_runtime_state_save_request(
    shared: &RuntimeRotationProxyShared,
    request: RuntimeStateSaveRequest,
) {
    let reason = request.reason.clone();
    let reason = reason.as_str();
    if !runtime_proxy_persistence_enabled(shared) {
        runtime_proxy_log(
            shared,
            format!(
                "state_save_suppressed role=follower reason={reason} path={}",
                request.paths.state_file.display()
            ),
        );
        return;
    }
    let revision = shared.state_save_revision.fetch_add(1, Ordering::SeqCst) + 1;
    let queued_at = Instant::now();
    let ready_at = queued_at + runtime_state_save_debounce(reason);
    let state_profiles = request.state.profiles.clone();
    let journal_continuations = runtime_state_save_reason_requires_continuation_journal(reason)
        .then(|| request.continuations.clone());
    if cfg!(test) {
        runtime_proxy_log(
            shared,
            format!(
                "state_save_inline revision={} reason={} ready_in_ms={}",
                revision,
                reason,
                ready_at.saturating_duration_since(queued_at).as_millis()
            ),
        );
        match save_runtime_state_snapshot_if_latest(
            &request.paths,
            &request.state,
            &request.continuations,
            &request.profile_scores,
            &request.usage_snapshots,
            &request.backoffs,
            revision,
            &shared.state_save_revision,
        ) {
            Ok(true) => runtime_proxy_log(
                shared,
                format!(
                    "state_save_ok revision={} reason={} lag_ms=0",
                    revision, reason
                ),
            ),
            Ok(false) => runtime_proxy_log(
                shared,
                format!(
                    "state_save_skipped revision={} reason={} lag_ms=0",
                    revision, reason
                ),
            ),
            Err(err) => runtime_proxy_log(
                shared,
                format!(
                    "state_save_error revision={} reason={} lag_ms=0 stage=write error={err:#}",
                    revision, reason
                ),
            ),
        }
        if let Some(continuations) = journal_continuations {
            schedule_runtime_continuation_journal_save(
                shared,
                continuations,
                request.state.profiles.clone(),
                request.paths,
                reason,
            );
        }
        return;
    }
    let backlog = enqueue_runtime_state_save_job(
        shared,
        request.paths.state_file.clone(),
        RuntimeStateSavePayload::Snapshot(RuntimeStateSaveSnapshot {
            paths: request.paths.clone(),
            state: request.state,
            continuations: request.continuations,
            profile_scores: request.profile_scores,
            usage_snapshots: request.usage_snapshots,
            backoffs: request.backoffs,
        }),
        revision,
        &request.reason,
        queued_at,
        ready_at,
    );
    runtime_proxy_log(
        shared,
        format!(
            "state_save_queued revision={} reason={} backlog={} ready_in_ms={}",
            revision,
            reason,
            backlog,
            ready_at.saturating_duration_since(queued_at).as_millis()
        ),
    );
    if runtime_proxy_queue_pressure_active(backlog, 0, 0) {
        runtime_proxy_log(
            shared,
            format!(
                "state_save_queue_backpressure revision={} reason={} backlog={backlog}",
                revision, reason
            ),
        );
    }
    if let Some(continuations) = journal_continuations {
        schedule_runtime_continuation_journal_save(
            shared,
            continuations,
            state_profiles,
            request.paths,
            reason,
        );
    }
}

pub(super) fn runtime_state_save_snapshot_from_runtime(
    runtime: &RuntimeRotationState,
) -> RuntimeStateSaveSnapshot {
    RuntimeStateSaveSnapshot {
        paths: runtime.paths.clone(),
        state: runtime.state.clone(),
        continuations: runtime_continuation_store_snapshot(runtime),
        profile_scores: runtime.profile_health.clone(),
        usage_snapshots: runtime.profile_usage_snapshots.clone(),
        backoffs: runtime_profile_backoffs_snapshot(runtime),
    }
}

pub(super) fn runtime_state_save_snapshot_from_shared(
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeStateSaveSnapshot> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime_state_save_snapshot_from_runtime(&runtime))
}

pub(super) fn schedule_runtime_state_save_from_runtime(
    shared: &RuntimeRotationProxyShared,
    runtime: &RuntimeRotationState,
    reason: &str,
) {
    if !runtime_proxy_persistence_enabled(shared) {
        return;
    }
    if cfg!(test) {
        schedule_runtime_state_save_request(
            shared,
            RuntimeStateSaveRequest::from_snapshot(
                runtime_state_save_snapshot_from_runtime(runtime),
                reason,
            ),
        );
        return;
    }
    let revision = shared.state_save_revision.fetch_add(1, Ordering::SeqCst) + 1;
    let queued_at = Instant::now();
    let ready_at = queued_at + runtime_state_save_debounce(reason);
    let backlog = enqueue_runtime_state_save_job(
        shared,
        runtime.paths.state_file.clone(),
        RuntimeStateSavePayload::Live(shared.clone()),
        revision,
        reason,
        queued_at,
        ready_at,
    );
    runtime_proxy_log(
        shared,
        format!(
            "state_save_queued revision={} reason={} backlog={} ready_in_ms={}",
            revision,
            reason,
            backlog,
            ready_at.saturating_duration_since(queued_at).as_millis()
        ),
    );
    if runtime_proxy_queue_pressure_active(backlog, 0, 0) {
        runtime_proxy_log(
            shared,
            format!(
                "state_save_queue_backpressure revision={} reason={} backlog={backlog}",
                revision, reason
            ),
        );
    }
    if runtime_state_save_reason_requires_continuation_journal(reason) {
        schedule_runtime_continuation_journal_save_from_runtime(shared, runtime, reason);
    }
}

fn enqueue_runtime_state_save_job(
    shared: &RuntimeRotationProxyShared,
    state_file: PathBuf,
    payload: RuntimeStateSavePayload,
    revision: u64,
    reason: &str,
    queued_at: Instant,
    ready_at: Instant,
) -> usize {
    let queue = runtime_state_save_queue();
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pending.insert(
        state_file,
        RuntimeStateSaveJob {
            payload,
            revision,
            latest_revision: Arc::clone(&shared.state_save_revision),
            log_path: shared.log_path.clone(),
            reason: reason.to_string(),
            queued_at,
            ready_at,
        },
    );
    let backlog = pending.len().saturating_sub(1);
    drop(pending);
    queue.wake.notify_one();
    backlog
}

pub(super) fn schedule_runtime_continuation_journal_save_from_runtime(
    shared: &RuntimeRotationProxyShared,
    runtime: &RuntimeRotationState,
    reason: &str,
) {
    if !runtime_proxy_persistence_enabled(shared) {
        return;
    }
    if cfg!(test) {
        schedule_runtime_continuation_journal_save(
            shared,
            runtime_continuation_store_snapshot(runtime),
            runtime.state.profiles.clone(),
            runtime.paths.clone(),
            reason,
        );
        return;
    }
    let queue = runtime_continuation_journal_save_queue();
    let journal_path = runtime_continuation_journal_file_path(&runtime.paths);
    let queued_at = Instant::now();
    let ready_at = queued_at + runtime_continuation_journal_save_debounce(reason);
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pending.insert(
        journal_path,
        RuntimeContinuationJournalSaveJob {
            payload: RuntimeContinuationJournalSavePayload::Live(shared.clone()),
            log_path: shared.log_path.clone(),
            reason: reason.to_string(),
            saved_at: Local::now().timestamp(),
            queued_at,
            ready_at,
        },
    );
    let backlog = pending.len().saturating_sub(1);
    drop(pending);
    queue.wake.notify_one();
    runtime_proxy_log(
        shared,
        format!(
            "continuation_journal_save_queued reason={} backlog={} ready_in_ms={}",
            reason,
            backlog,
            ready_at.saturating_duration_since(queued_at).as_millis()
        ),
    );
    if runtime_proxy_queue_pressure_active(0, backlog, 0) {
        runtime_proxy_log(
            shared,
            format!(
                "continuation_journal_queue_backpressure reason={} backlog={backlog}",
                reason
            ),
        );
    }
}

pub(super) fn runtime_state_save_queue() -> Arc<RuntimeStateSaveQueue> {
    Arc::clone(RUNTIME_STATE_SAVE_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeStateSaveQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
            active: Arc::new(AtomicUsize::new(0)),
        });
        let worker_queue = Arc::clone(&queue);
        thread::spawn(move || runtime_state_save_worker_loop(worker_queue));
        queue
    }))
}

pub(super) fn runtime_continuation_journal_save_queue() -> Arc<RuntimeContinuationJournalSaveQueue>
{
    Arc::clone(RUNTIME_CONTINUATION_JOURNAL_SAVE_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeContinuationJournalSaveQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
            active: Arc::new(AtomicUsize::new(0)),
        });
        let worker_queue = Arc::clone(&queue);
        thread::spawn(move || runtime_continuation_journal_save_worker_loop(worker_queue));
        queue
    }))
}

pub(super) fn runtime_probe_refresh_queue() -> Arc<RuntimeProbeRefreshQueue> {
    Arc::clone(RUNTIME_PROBE_REFRESH_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeProbeRefreshQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
            active: Arc::new(AtomicUsize::new(0)),
            wait: Arc::new((Mutex::new(()), Condvar::new())),
            revision: Arc::new(AtomicU64::new(0)),
        });
        for _ in 0..runtime_probe_refresh_worker_count() {
            let worker_queue = Arc::clone(&queue);
            thread::spawn(move || runtime_probe_refresh_worker_loop(worker_queue));
        }
        queue
    }))
}

pub(super) fn runtime_state_save_queue_backlog() -> usize {
    runtime_state_save_queue()
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

#[allow(dead_code)]
pub(super) fn runtime_state_save_queue_active() -> usize {
    runtime_state_save_queue().active.load(Ordering::SeqCst)
}

pub(super) fn runtime_continuation_journal_queue_backlog() -> usize {
    runtime_continuation_journal_save_queue()
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

#[allow(dead_code)]
pub(super) fn runtime_continuation_journal_queue_active() -> usize {
    runtime_continuation_journal_save_queue()
        .active
        .load(Ordering::SeqCst)
}

pub(super) fn runtime_probe_refresh_queue_backlog() -> usize {
    runtime_probe_refresh_queue()
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

pub(super) fn runtime_probe_refresh_revision() -> u64 {
    runtime_probe_refresh_queue()
        .revision
        .load(Ordering::SeqCst)
}

pub(super) fn note_runtime_probe_refresh_progress() {
    let queue = runtime_probe_refresh_queue();
    queue.revision.fetch_add(1, Ordering::SeqCst);
    let (mutex, condvar) = &*queue.wait;
    let _guard = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    condvar.notify_all();
}

#[allow(dead_code)]
pub(super) fn runtime_probe_refresh_queue_active() -> usize {
    runtime_probe_refresh_queue().active.load(Ordering::SeqCst)
}

pub(super) fn runtime_proxy_queue_pressure_active(
    state_save_backlog: usize,
    continuation_journal_backlog: usize,
    probe_refresh_backlog: usize,
) -> bool {
    state_save_backlog >= RUNTIME_STATE_SAVE_QUEUE_PRESSURE_THRESHOLD
        || continuation_journal_backlog >= RUNTIME_CONTINUATION_JOURNAL_QUEUE_PRESSURE_THRESHOLD
        || probe_refresh_backlog >= RUNTIME_PROBE_REFRESH_QUEUE_PRESSURE_THRESHOLD
}

pub(super) fn schedule_runtime_continuation_journal_save(
    shared: &RuntimeRotationProxyShared,
    continuations: RuntimeContinuationStore,
    profiles: BTreeMap<String, ProfileEntry>,
    paths: AppPaths,
    reason: &str,
) {
    if !runtime_proxy_persistence_enabled(shared) {
        runtime_proxy_log(
            shared,
            format!(
                "continuation_journal_save_suppressed role=follower reason={reason} path={}",
                runtime_continuation_journal_file_path(&paths).display()
            ),
        );
        return;
    }
    if cfg!(test) {
        runtime_proxy_log(
            shared,
            format!("continuation_journal_save_inline reason={reason} backlog=0"),
        );
        let saved_at = Local::now().timestamp();
        match save_runtime_continuation_journal_for_profiles(
            &paths,
            &continuations,
            &profiles,
            saved_at,
        ) {
            Ok(()) => runtime_proxy_log(
                shared,
                format!(
                    "continuation_journal_save_ok saved_at={} reason={} lag_ms=0",
                    saved_at, reason
                ),
            ),
            Err(err) => runtime_proxy_log(
                shared,
                format!(
                    "continuation_journal_save_error saved_at={} reason={} lag_ms=0 stage=write error={err:#}",
                    saved_at, reason
                ),
            ),
        }
        return;
    }
    let queue = runtime_continuation_journal_save_queue();
    let journal_path = runtime_continuation_journal_file_path(&paths);
    let queued_at = Instant::now();
    let ready_at = queued_at + runtime_continuation_journal_save_debounce(reason);
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pending.insert(
        journal_path,
        RuntimeContinuationJournalSaveJob {
            payload: RuntimeContinuationJournalSavePayload::Snapshot(
                RuntimeContinuationJournalSnapshot {
                    paths,
                    continuations,
                    profiles,
                },
            ),
            log_path: shared.log_path.clone(),
            reason: reason.to_string(),
            saved_at: Local::now().timestamp(),
            queued_at,
            ready_at,
        },
    );
    let backlog = pending.len().saturating_sub(1);
    drop(pending);
    queue.wake.notify_one();
    runtime_proxy_log(
        shared,
        format!(
            "continuation_journal_save_queued reason={} backlog={} ready_in_ms={}",
            reason,
            backlog,
            ready_at.saturating_duration_since(queued_at).as_millis()
        ),
    );
    if runtime_proxy_queue_pressure_active(0, backlog, 0) {
        runtime_proxy_log(
            shared,
            format!(
                "continuation_journal_queue_backpressure reason={} backlog={backlog}",
                reason
            ),
        );
    }
}

pub(super) fn runtime_continuation_journal_snapshot_from_runtime(
    runtime: &RuntimeRotationState,
) -> RuntimeContinuationJournalSnapshot {
    RuntimeContinuationJournalSnapshot {
        paths: runtime.paths.clone(),
        continuations: runtime_continuation_store_snapshot(runtime),
        profiles: runtime.state.profiles.clone(),
    }
}

pub(super) fn runtime_continuation_journal_snapshot_from_shared(
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeContinuationJournalSnapshot> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime_continuation_journal_snapshot_from_runtime(&runtime))
}

pub(super) fn runtime_state_save_worker_loop(queue: Arc<RuntimeStateSaveQueue>) {
    runtime_run_scheduled_save_worker_loop(
        &queue.pending,
        &queue.wake,
        queue.active.as_ref(),
        |job| {
            let snapshot = match &job.payload {
                RuntimeStateSavePayload::Snapshot(snapshot) => Ok(snapshot.clone()),
                RuntimeStateSavePayload::Live(shared) => {
                    runtime_state_save_snapshot_from_shared(shared)
                }
            };
            match snapshot.and_then(|snapshot| {
                save_runtime_state_snapshot_if_latest(
                    &snapshot.paths,
                    &snapshot.state,
                    &snapshot.continuations,
                    &snapshot.profile_scores,
                    &snapshot.usage_snapshots,
                    &snapshot.backoffs,
                    job.revision,
                    &job.latest_revision,
                )
            }) {
                Ok(true) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "state_save_ok revision={} reason={} lag_ms={}",
                        job.revision,
                        job.reason,
                        job.queued_at.elapsed().as_millis()
                    ),
                ),
                Ok(false) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "state_save_skipped revision={} reason={} lag_ms={}",
                        job.revision,
                        job.reason,
                        job.queued_at.elapsed().as_millis()
                    ),
                ),
                Err(err) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "state_save_error revision={} reason={} lag_ms={} stage=write error={err:#}",
                        job.revision,
                        job.reason,
                        job.queued_at.elapsed().as_millis()
                    ),
                ),
            }
        },
    )
}

pub(super) fn runtime_continuation_journal_save_worker_loop(
    queue: Arc<RuntimeContinuationJournalSaveQueue>,
) {
    runtime_run_scheduled_save_worker_loop(
        &queue.pending,
        &queue.wake,
        queue.active.as_ref(),
        |job| {
            let snapshot = match &job.payload {
                RuntimeContinuationJournalSavePayload::Snapshot(snapshot) => Ok(snapshot.clone()),
                RuntimeContinuationJournalSavePayload::Live(shared) => {
                    runtime_continuation_journal_snapshot_from_shared(shared)
                }
            };
            match snapshot.and_then(|snapshot| {
                save_runtime_continuation_journal_for_profiles(
                    &snapshot.paths,
                    &snapshot.continuations,
                    &snapshot.profiles,
                    job.saved_at,
                )
            }) {
                Ok(()) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "continuation_journal_save_ok saved_at={} reason={} lag_ms={}",
                        job.saved_at,
                        job.reason,
                        job.queued_at.elapsed().as_millis()
                    ),
                ),
                Err(err) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "continuation_journal_save_error saved_at={} reason={} lag_ms={} stage=write error={err:#}",
                        job.saved_at,
                        job.reason,
                        job.queued_at.elapsed().as_millis()
                    ),
                ),
            }
        },
    )
}

fn runtime_wait_for_due_scheduled_jobs<K, J>(
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

fn runtime_run_scheduled_save_worker_loop<K, J, F>(
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

pub(super) fn runtime_state_save_reason_requires_continuation_journal(reason: &str) -> bool {
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

pub(super) fn runtime_hot_continuation_state_reason(reason: &str) -> bool {
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

pub(super) fn runtime_state_save_debounce(reason: &str) -> Duration {
    if runtime_hot_continuation_state_reason(reason) {
        Duration::from_millis(RUNTIME_STATE_SAVE_DEBOUNCE_MS)
    } else {
        Duration::ZERO
    }
}

pub(super) fn runtime_continuation_journal_save_debounce(reason: &str) -> Duration {
    if runtime_hot_continuation_state_reason(reason) {
        Duration::from_millis(RUNTIME_STATE_SAVE_DEBOUNCE_MS)
    } else {
        Duration::ZERO
    }
}

pub(super) fn schedule_runtime_probe_refresh(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    codex_home: &Path,
) {
    let (state_file, upstream_base_url) = match shared.runtime.lock() {
        Ok(runtime) => (
            runtime.paths.state_file.clone(),
            runtime.upstream_base_url.clone(),
        ),
        Err(_) => return,
    };
    #[cfg(test)]
    if runtime_probe_refresh_nonlocal_upstream_for_test(&upstream_base_url) {
        runtime_proxy_log(
            shared,
            format!(
                "profile_probe_refresh_suppressed profile={profile_name} reason=test_nonlocal_upstream"
            ),
        );
        note_runtime_probe_refresh_progress();
        return;
    }

    let queue = runtime_probe_refresh_queue();
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if pending.contains_key(&(state_file.clone(), profile_name.to_string())) {
        return;
    }
    let queued_at = Instant::now();
    pending.insert(
        (state_file.clone(), profile_name.to_string()),
        RuntimeProbeRefreshJob {
            shared: shared.clone(),
            profile_name: profile_name.to_string(),
            codex_home: codex_home.to_path_buf(),
            upstream_base_url,
            queued_at,
        },
    );
    let backlog = pending.len().saturating_sub(1);
    drop(pending);
    queue.wake.notify_one();
    runtime_proxy_log(
        shared,
        format!(
            "profile_probe_refresh_queued profile={profile_name} reason=queued backlog={backlog}"
        ),
    );
    if runtime_proxy_queue_pressure_active(0, 0, backlog) {
        runtime_proxy_log(
            shared,
            format!("profile_probe_refresh_backpressure profile={profile_name} backlog={backlog}"),
        );
    }
}

#[cfg(test)]
pub(super) fn runtime_probe_refresh_nonlocal_upstream_for_test(upstream_base_url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(upstream_base_url) else {
        return true;
    };
    !matches!(
        url.host_str(),
        Some("127.0.0.1") | Some("localhost") | Some("::1") | Some("[::1]")
    )
}

pub(super) fn runtime_profiles_needing_startup_probe_refresh(
    state: &AppState,
    current_profile: &str,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    profile_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
) -> Vec<String> {
    active_profile_selection_order(state, current_profile)
        .into_iter()
        .filter(|profile_name| {
            let probe_fresh = profile_probe_cache.get(profile_name).is_some_and(|entry| {
                runtime_profile_probe_cache_freshness(entry, now)
                    == RuntimeProbeCacheFreshness::Fresh
            });
            let snapshot_usable = profile_usage_snapshots
                .get(profile_name)
                .is_some_and(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now));
            !probe_fresh && !snapshot_usable
        })
        .take(RUNTIME_STARTUP_PROBE_WARM_LIMIT)
        .collect()
}

pub(super) fn run_runtime_probe_jobs_inline(
    shared: &RuntimeRotationProxyShared,
    jobs: Vec<(String, PathBuf)>,
    context: &str,
) {
    if jobs.is_empty() {
        return;
    }
    let upstream_base_url = match shared.runtime.lock() {
        Ok(runtime) => runtime.upstream_base_url.clone(),
        Err(_) => return,
    };
    let probe_reports = map_parallel(jobs, |(profile_name, codex_home)| {
        (
            profile_name,
            RuntimeProbeRefreshAttempt::collect(&codex_home, upstream_base_url.as_str()),
        )
    });
    for (profile_name, attempt) in probe_reports {
        attempt.execute(
            RuntimeProbeExecutionMode::Inline { context },
            shared,
            &profile_name,
            Instant::now(),
        );
    }
}

pub(super) fn schedule_runtime_startup_probe_warmup(shared: &RuntimeRotationProxyShared) {
    if runtime_proxy_pressure_mode_active(shared) {
        runtime_proxy_log(
            shared,
            "startup_probe_warmup deferred reason=local_pressure",
        );
        return;
    }
    let (state, current_profile, profile_probe_cache, profile_usage_snapshots) =
        match shared.runtime.lock() {
            Ok(runtime) => (
                runtime.state.clone(),
                runtime.current_profile.clone(),
                runtime.profile_probe_cache.clone(),
                runtime.profile_usage_snapshots.clone(),
            ),
            Err(_) => return,
        };
    let refresh_profiles = runtime_profiles_needing_startup_probe_refresh(
        &state,
        &current_profile,
        &profile_probe_cache,
        &profile_usage_snapshots,
        Local::now().timestamp(),
    );
    if refresh_profiles.is_empty() {
        return;
    }

    let refresh_jobs = refresh_profiles
        .into_iter()
        .filter_map(|profile_name| {
            let profile = state.profiles.get(&profile_name)?;
            profile
                .provider
                .auth_summary(&profile.codex_home)
                .quota_compatible
                .then(|| (profile_name, profile.codex_home.clone()))
        })
        .collect::<Vec<_>>();
    if refresh_jobs.is_empty() {
        return;
    }

    let sync_limit = runtime_startup_sync_probe_warm_limit();
    let sync_count = if cfg!(test) {
        refresh_jobs.len()
    } else {
        sync_limit.min(refresh_jobs.len())
    };
    if sync_count > 0 {
        let sync_jobs = refresh_jobs
            .iter()
            .take(sync_count)
            .cloned()
            .collect::<Vec<_>>();
        runtime_proxy_log(
            shared,
            format!(
                "startup_probe_warmup sync={} profiles={}",
                sync_jobs.len(),
                sync_jobs
                    .iter()
                    .map(|(profile_name, _)| profile_name.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        );
        run_runtime_probe_jobs_inline(shared, sync_jobs, "startup_probe_warmup");
    }

    if cfg!(test) {
        return;
    }
    let async_jobs = refresh_jobs
        .into_iter()
        .skip(sync_count)
        .collect::<Vec<_>>();
    if async_jobs.is_empty() {
        return;
    }
    runtime_proxy_log(
        shared,
        format!(
            "startup_probe_warmup queued={} profiles={}",
            async_jobs.len(),
            async_jobs
                .iter()
                .map(|(profile_name, _)| profile_name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ),
    );
    for (profile_name, codex_home) in async_jobs {
        schedule_runtime_probe_refresh(shared, &profile_name, &codex_home);
    }
}

pub(super) fn apply_runtime_profile_probe_result(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
) -> Result<()> {
    let _progress = RuntimeProbeProgressObserver::new();
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let (log_messages, state_save_args) = apply_runtime_profile_probe_result_to_runtime(
        &mut runtime,
        profile_name,
        auth,
        result,
        Local::now().timestamp(),
    );
    drop(runtime);
    emit_runtime_profile_probe_result(shared, log_messages, state_save_args);
    Ok(())
}

fn apply_runtime_profile_probe_result_to_runtime(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
    now: i64,
) -> (Vec<String>, Option<RuntimeStateSaveRequest>) {
    let mut log_messages = Vec::new();
    let mut state_save_args = None;
    runtime.profile_probe_cache.insert(
        profile_name.to_string(),
        RuntimeProfileProbeCacheEntry {
            checked_at: now,
            auth,
            result: result.clone(),
        },
    );

    if let Ok(usage) = result {
        let snapshot = runtime_profile_usage_snapshot_from_usage(&usage);
        let previous_snapshot = runtime.profile_usage_snapshots.get(profile_name).cloned();
        let previous_retry_backoff = runtime
            .profile_retry_backoff_until
            .get(profile_name)
            .copied();
        let quota_summary =
            runtime_quota_summary_from_usage_snapshot(&snapshot, RuntimeRouteKind::Responses);
        let blocking_reset_at =
            runtime_quota_summary_blocking_reset_at(quota_summary, RuntimeRouteKind::Responses)
                .filter(|reset_at| *reset_at > now);
        let quarantine_until =
            runtime_quota_precommit_guard_reason(quota_summary, RuntimeRouteKind::Responses).map(
                |_| {
                    blocking_reset_at.unwrap_or_else(|| {
                        now.saturating_add(RUNTIME_PROFILE_QUOTA_QUARANTINE_FALLBACK_SECONDS)
                    })
                },
            );
        let mut quarantine_applied = None;
        if let Some(until) = quarantine_until {
            let next_until = runtime
                .profile_retry_backoff_until
                .get(profile_name)
                .copied()
                .unwrap_or(until)
                .max(until);
            runtime
                .profile_retry_backoff_until
                .insert(profile_name.to_string(), next_until);
            quarantine_applied = Some(next_until);
        }
        let snapshot_should_persist = runtime_profile_usage_snapshot_should_persist(
            previous_snapshot.as_ref(),
            &snapshot,
            now,
        );
        let retry_backoff_changed = runtime
            .profile_retry_backoff_until
            .get(profile_name)
            .copied()
            != previous_retry_backoff;
        runtime
            .profile_usage_snapshots
            .insert(profile_name.to_string(), snapshot);
        if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
            log_messages.push(format!(
                "quota_probe_exhausted profile={profile_name} reason=usage_snapshot_exhausted {}",
                runtime_quota_summary_log_fields(quota_summary)
            ));
        }
        if let Some(until) = quarantine_applied
            && retry_backoff_changed
        {
            log_messages.push(format!(
                "profile_quota_quarantine profile={profile_name} route={} until={} reset_at={} message=probe_snapshot",
                runtime_route_kind_label(RuntimeRouteKind::Responses),
                until,
                blocking_reset_at.unwrap_or(i64::MAX),
            ));
            log_messages.push(format!(
                "profile_retry_backoff profile={profile_name} until={until}",
            ));
        }
        if snapshot_should_persist || retry_backoff_changed {
            state_save_args = Some(RuntimeStateSaveRequest::from_snapshot(
                runtime_state_save_snapshot_from_runtime(runtime),
                &format!("usage_snapshot:{profile_name}"),
            ));
        }
    }

    (log_messages, state_save_args)
}

fn emit_runtime_profile_probe_result(
    shared: &RuntimeRotationProxyShared,
    log_messages: Vec<String>,
    state_save_args: Option<RuntimeStateSaveRequest>,
) {
    for message in log_messages {
        runtime_proxy_log(shared, message);
    }
    if let Some(request) = state_save_args {
        schedule_runtime_state_save_request(shared, request);
    }
}

fn runtime_probe_refresh_apply_wait_timeout() -> Duration {
    Duration::from_millis(if cfg!(test) { 250 } else { 1_000 })
}

fn apply_runtime_profile_probe_result_with_timeout(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
    timeout: Duration,
) -> Result<()> {
    let _progress = RuntimeProbeProgressObserver::new();
    let started_at = Instant::now();
    let now = Local::now().timestamp();
    loop {
        match shared.runtime.try_lock() {
            Ok(mut runtime) => {
                let (log_messages, state_save_args) = apply_runtime_profile_probe_result_to_runtime(
                    &mut runtime,
                    profile_name,
                    auth,
                    result,
                    now,
                );
                drop(runtime);
                emit_runtime_profile_probe_result(shared, log_messages, state_save_args);
                return Ok(());
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(anyhow::anyhow!("runtime auto-rotate state is poisoned"));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                if started_at.elapsed() >= timeout {
                    return Err(anyhow::anyhow!(
                        "runtime auto-rotate state remained busy during probe apply"
                    ));
                }
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

pub(super) fn runtime_profile_usage_snapshot_materially_matches(
    previous: &RuntimeProfileUsageSnapshot,
    next: &RuntimeProfileUsageSnapshot,
) -> bool {
    previous.five_hour_status == next.five_hour_status
        && previous.five_hour_remaining_percent == next.five_hour_remaining_percent
        && previous.five_hour_reset_at == next.five_hour_reset_at
        && previous.weekly_status == next.weekly_status
        && previous.weekly_remaining_percent == next.weekly_remaining_percent
        && previous.weekly_reset_at == next.weekly_reset_at
}

pub(super) fn runtime_profile_usage_snapshot_should_persist(
    previous: Option<&RuntimeProfileUsageSnapshot>,
    next: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    let Some(previous) = previous else {
        return true;
    };

    !runtime_profile_usage_snapshot_materially_matches(previous, next)
        || runtime_binding_touch_should_persist(previous.checked_at, now)
}

pub(super) fn runtime_probe_refresh_worker_loop(queue: Arc<RuntimeProbeRefreshQueue>) {
    loop {
        let jobs = {
            let mut pending = queue
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while pending.is_empty() {
                pending = queue
                    .wake
                    .wait(pending)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            std::mem::take(&mut *pending)
        };

        for (_, job) in jobs {
            queue.active.fetch_add(1, Ordering::SeqCst);
            let log_path = job.shared.log_path.clone();
            let panic_result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job.execute()));
            queue.active.fetch_sub(1, Ordering::SeqCst);
            if let Err(panic_payload) = panic_result {
                let panic_message = if let Some(message) = panic_payload.downcast_ref::<&str>() {
                    (*message).to_string()
                } else if let Some(message) = panic_payload.downcast_ref::<String>() {
                    message.clone()
                } else {
                    "unknown panic payload".to_string()
                };
                runtime_proxy_log_to_path(
                    &log_path,
                    &format!("profile_probe_refresh_panic error={panic_message}"),
                );
            }
        }
    }
}

struct RuntimeProbeProgressObserver;

impl RuntimeProbeProgressObserver {
    fn new() -> Self {
        Self
    }
}

impl Drop for RuntimeProbeProgressObserver {
    fn drop(&mut self) {
        note_runtime_probe_refresh_progress();
    }
}

enum RuntimeProbeExecutionMode<'a> {
    Inline { context: &'a str },
    Queued { apply_timeout: Duration },
}

impl RuntimeProbeExecutionMode<'_> {
    fn apply(
        &self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        auth: AuthSummary,
        result: std::result::Result<UsageResponse, String>,
    ) -> Result<()> {
        match self {
            Self::Inline { .. } => {
                apply_runtime_profile_probe_result(shared, profile_name, auth, result)
            }
            Self::Queued { apply_timeout } => apply_runtime_profile_probe_result_with_timeout(
                shared,
                profile_name,
                auth,
                result,
                *apply_timeout,
            ),
        }
    }

    fn log_completion(
        &self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        queued_at: Instant,
        result: &std::result::Result<UsageResponse, String>,
        apply_result: Result<()>,
    ) {
        match self {
            Self::Inline { context } => match result {
                Ok(_) => runtime_proxy_log(
                    shared,
                    if let Err(err) = apply_result {
                        format!(
                            "{}_error profile={} error=state_update:{err:#}",
                            context, profile_name
                        )
                    } else {
                        format!("{}_ok profile={profile_name}", context)
                    },
                ),
                Err(err) => runtime_proxy_log(
                    shared,
                    format!("{}_error profile={} error={err}", context, profile_name),
                ),
            },
            Self::Queued { .. } => {
                let lag_ms = queued_at.elapsed().as_millis();
                match result {
                    Ok(_) => runtime_proxy_log(
                        shared,
                        if let Err(err) = apply_result {
                            format!(
                                "profile_probe_refresh_error profile={} lag_ms={} error=state_update:{err:#}",
                                profile_name, lag_ms
                            )
                        } else {
                            format!(
                                "profile_probe_refresh_ok profile={} lag_ms={lag_ms}",
                                profile_name
                            )
                        },
                    ),
                    Err(err) => runtime_proxy_log(
                        shared,
                        format!(
                            "profile_probe_refresh_error profile={} lag_ms={} error={err}",
                            profile_name, lag_ms
                        ),
                    ),
                }
            }
        }
    }
}

struct RuntimeProbeRefreshAttempt {
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
}

impl RuntimeProbeRefreshAttempt {
    fn collect(codex_home: &Path, upstream_base_url: &str) -> Self {
        let auth = read_auth_summary(codex_home);
        let result = if auth.quota_compatible {
            fetch_usage(codex_home, Some(upstream_base_url)).map_err(|err| err.to_string())
        } else {
            Err("auth mode is not quota-compatible".to_string())
        };
        Self { auth, result }
    }

    fn execute(
        self,
        mode: RuntimeProbeExecutionMode<'_>,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        queued_at: Instant,
    ) {
        let apply_result = mode.apply(shared, profile_name, self.auth, self.result.clone());
        mode.log_completion(shared, profile_name, queued_at, &self.result, apply_result);
    }
}

#[cfg(test)]
pub(super) fn execute_runtime_probe_attempt_inline_for_test(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &str,
    result: std::result::Result<UsageResponse, String>,
) {
    RuntimeProbeRefreshAttempt {
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result,
    }
    .execute(
        RuntimeProbeExecutionMode::Inline { context },
        shared,
        profile_name,
        Instant::now(),
    );
}

#[cfg(test)]
pub(super) fn execute_runtime_probe_attempt_queued_for_test(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    result: std::result::Result<UsageResponse, String>,
    apply_timeout: Duration,
    queued_at: Instant,
) {
    RuntimeProbeRefreshAttempt {
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result,
    }
    .execute(
        RuntimeProbeExecutionMode::Queued { apply_timeout },
        shared,
        profile_name,
        queued_at,
    );
}

impl RuntimeProbeRefreshJob {
    fn execute(self) {
        runtime_proxy_log(
            &self.shared,
            format!("profile_probe_refresh_start profile={}", self.profile_name),
        );
        RuntimeProbeRefreshAttempt::collect(&self.codex_home, self.upstream_base_url.as_str())
            .execute(
                RuntimeProbeExecutionMode::Queued {
                    apply_timeout: runtime_probe_refresh_apply_wait_timeout(),
                },
                &self.shared,
                &self.profile_name,
                self.queued_at,
            );
    }
}
