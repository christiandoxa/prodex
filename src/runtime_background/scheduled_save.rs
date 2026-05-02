use super::*;

pub(crate) struct RuntimeStateSaveRequest {
    state: AppState,
    continuations: RuntimeContinuationStore,
    profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: RuntimeProfileBackoffs,
    paths: AppPaths,
    reason: String,
}

impl RuntimeStateSaveRequest {
    pub(crate) fn from_snapshot(snapshot: RuntimeStateSaveSnapshot, reason: &str) -> Self {
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
pub(crate) fn schedule_runtime_state_save(
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

pub(crate) fn schedule_runtime_state_save_request(
    shared: &RuntimeRotationProxyShared,
    request: RuntimeStateSaveRequest,
) {
    let reason = request.reason.clone();
    let reason = reason.as_str();
    if !runtime_proxy_persistence_enabled(shared) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "state_save_suppressed",
                [
                    runtime_proxy_log_field("role", "follower"),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field("path", request.paths.state_file.display().to_string()),
                ],
            ),
        );
        return;
    }
    let revision = shared.state_save_revision.fetch_add(1, Ordering::SeqCst) + 1;
    let enqueue_plan = runtime_state_save_enqueue_plan(reason, Instant::now());
    let plan = enqueue_plan.schedule;
    let queued_at = enqueue_plan.queue.queued_at;
    let ready_at = enqueue_plan.queue.ready_at;
    let state_profiles = request.state.profiles.clone();
    let journal_continuations = plan
        .requires_continuation_journal
        .then(|| request.continuations.clone());
    if cfg!(test) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "state_save_inline",
                [
                    runtime_proxy_log_field("revision", revision.to_string()),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field(
                        "ready_in_ms",
                        enqueue_plan.queue.ready_in_ms().to_string(),
                    ),
                ],
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
                runtime_proxy_structured_log_message(
                    "state_save_ok",
                    [
                        runtime_proxy_log_field("revision", revision.to_string()),
                        runtime_proxy_log_field("reason", reason),
                        runtime_proxy_log_field("lag_ms", "0"),
                    ],
                ),
            ),
            Ok(false) => runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "state_save_skipped",
                    [
                        runtime_proxy_log_field("revision", revision.to_string()),
                        runtime_proxy_log_field("reason", reason),
                        runtime_proxy_log_field("lag_ms", "0"),
                    ],
                ),
            ),
            Err(err) => runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "state_save_error",
                    [
                        runtime_proxy_log_field("revision", revision.to_string()),
                        runtime_proxy_log_field("reason", reason),
                        runtime_proxy_log_field("lag_ms", "0"),
                        runtime_proxy_log_field("stage", "write"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
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
    let queue_plan = enqueue_runtime_state_save_job(
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
    let backlog = queue_plan.backlog;
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "state_save_queued",
            [
                runtime_proxy_log_field("revision", revision.to_string()),
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("backlog", backlog.to_string()),
                runtime_proxy_log_field(
                    "ready_in_ms",
                    enqueue_plan.queue.ready_in_ms().to_string(),
                ),
            ],
        ),
    );
    if queue_plan.pressure_active {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "state_save_queue_backpressure",
                [
                    runtime_proxy_log_field("revision", revision.to_string()),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field("backlog", backlog.to_string()),
                ],
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

pub(crate) fn runtime_state_save_snapshot_from_runtime(
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

pub(crate) fn runtime_state_save_selected_snapshot_from_runtime(
    runtime: &RuntimeRotationState,
    sections: RuntimeStateSaveSections,
) -> RuntimeStateSaveSelectedSnapshot {
    let continuations = if sections.continuations {
        runtime_continuation_store_snapshot(runtime)
    } else {
        RuntimeContinuationStore::default()
    };
    let backoffs = if sections.backoffs {
        runtime_profile_backoffs_snapshot(runtime)
    } else {
        RuntimeProfileBackoffs::default()
    };
    prodex_runtime_store::runtime_state_save_selected_snapshot_from_parts(
        &runtime.paths,
        &runtime.state,
        &continuations,
        &runtime.profile_health,
        &runtime.profile_usage_snapshots,
        &backoffs,
        sections,
    )
}

pub(crate) fn runtime_state_save_selected_snapshot_from_shared(
    shared: &RuntimeRotationProxyShared,
    sections: RuntimeStateSaveSections,
) -> Result<RuntimeStateSaveSelectedSnapshot> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    if sections.continuations {
        compact_runtime_continuation_state_in_place(&mut runtime);
    }
    Ok(runtime_state_save_selected_snapshot_from_runtime(
        &runtime, sections,
    ))
}

pub(crate) fn schedule_runtime_state_save_from_runtime(
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
    let enqueue_plan = runtime_state_save_enqueue_plan(reason, Instant::now());
    let plan = enqueue_plan.schedule;
    let queued_at = enqueue_plan.queue.queued_at;
    let ready_at = enqueue_plan.queue.ready_at;
    let queue_plan = enqueue_runtime_state_save_job(
        shared,
        runtime.paths.state_file.clone(),
        RuntimeStateSavePayload::Live {
            shared: shared.clone(),
            sections: plan.sections,
        },
        revision,
        reason,
        queued_at,
        ready_at,
    );
    let backlog = queue_plan.backlog;
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "state_save_queued",
            [
                runtime_proxy_log_field("revision", revision.to_string()),
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("backlog", backlog.to_string()),
                runtime_proxy_log_field(
                    "ready_in_ms",
                    enqueue_plan.queue.ready_in_ms().to_string(),
                ),
            ],
        ),
    );
    if queue_plan.pressure_active {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "state_save_queue_backpressure",
                [
                    runtime_proxy_log_field("revision", revision.to_string()),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field("backlog", backlog.to_string()),
                ],
            ),
        );
    }
    if plan.requires_continuation_journal {
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
) -> prodex_runtime_state::RuntimeBackgroundQueueEnqueuePlan {
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
    let plan = runtime_background_queue_enqueue_plan(
        prodex_runtime_state::RuntimeBackgroundQueueKind::StateSave,
        pending.len(),
    );
    drop(pending);
    queue.wake.notify_one();
    plan
}

pub(crate) fn schedule_runtime_continuation_journal_save_from_runtime(
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
    let enqueue_plan = runtime_continuation_journal_save_enqueue_plan(reason, Instant::now());
    let queued_at = enqueue_plan.queued_at;
    let ready_at = enqueue_plan.ready_at;
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
    let queue_plan = runtime_background_queue_enqueue_plan(
        prodex_runtime_state::RuntimeBackgroundQueueKind::ContinuationJournal,
        pending.len(),
    );
    drop(pending);
    queue.wake.notify_one();
    let backlog = queue_plan.backlog;
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "continuation_journal_save_queued",
            [
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("backlog", backlog.to_string()),
                runtime_proxy_log_field("ready_in_ms", enqueue_plan.ready_in_ms().to_string()),
            ],
        ),
    );
    if queue_plan.pressure_active {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "continuation_journal_queue_backpressure",
                [
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field("backlog", backlog.to_string()),
                ],
            ),
        );
    }
}

pub(crate) fn runtime_state_save_queue() -> Arc<RuntimeStateSaveQueue> {
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

pub(crate) fn runtime_continuation_journal_save_queue() -> Arc<RuntimeContinuationJournalSaveQueue>
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

pub(crate) fn runtime_state_save_queue_backlog() -> usize {
    runtime_state_save_queue()
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

pub(crate) fn schedule_runtime_continuation_journal_save(
    shared: &RuntimeRotationProxyShared,
    continuations: RuntimeContinuationStore,
    profiles: BTreeMap<String, ProfileEntry>,
    paths: AppPaths,
    reason: &str,
) {
    if !runtime_proxy_persistence_enabled(shared) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "continuation_journal_save_suppressed",
                [
                    runtime_proxy_log_field("role", "follower"),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field(
                        "path",
                        runtime_continuation_journal_file_path(&paths)
                            .display()
                            .to_string(),
                    ),
                ],
            ),
        );
        return;
    }
    if cfg!(test) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "continuation_journal_save_inline",
                [
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field("backlog", "0"),
                ],
            ),
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
                runtime_proxy_structured_log_message(
                    "continuation_journal_save_ok",
                    [
                        runtime_proxy_log_field("saved_at", saved_at.to_string()),
                        runtime_proxy_log_field("reason", reason),
                        runtime_proxy_log_field("lag_ms", "0"),
                    ],
                ),
            ),
            Err(err) => runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "continuation_journal_save_error",
                    [
                        runtime_proxy_log_field("saved_at", saved_at.to_string()),
                        runtime_proxy_log_field("reason", reason),
                        runtime_proxy_log_field("lag_ms", "0"),
                        runtime_proxy_log_field("stage", "write"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            ),
        }
        return;
    }
    let queue = runtime_continuation_journal_save_queue();
    let journal_path = runtime_continuation_journal_file_path(&paths);
    let enqueue_plan = runtime_continuation_journal_save_enqueue_plan(reason, Instant::now());
    let queued_at = enqueue_plan.queued_at;
    let ready_at = enqueue_plan.ready_at;
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
    let queue_plan = runtime_background_queue_enqueue_plan(
        prodex_runtime_state::RuntimeBackgroundQueueKind::ContinuationJournal,
        pending.len(),
    );
    drop(pending);
    queue.wake.notify_one();
    let backlog = queue_plan.backlog;
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "continuation_journal_save_queued",
            [
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("backlog", backlog.to_string()),
                runtime_proxy_log_field("ready_in_ms", enqueue_plan.ready_in_ms().to_string()),
            ],
        ),
    );
    if queue_plan.pressure_active {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "continuation_journal_queue_backpressure",
                [
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field("backlog", backlog.to_string()),
                ],
            ),
        );
    }
}

pub(crate) fn runtime_continuation_journal_snapshot_from_runtime(
    runtime: &RuntimeRotationState,
) -> RuntimeContinuationJournalSnapshot {
    RuntimeContinuationJournalSnapshot {
        paths: runtime.paths.clone(),
        continuations: runtime_continuation_store_snapshot(runtime),
        profiles: runtime.state.profiles.clone(),
    }
}

pub(crate) fn runtime_continuation_journal_snapshot_from_shared(
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeContinuationJournalSnapshot> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    compact_runtime_continuation_state_in_place(&mut runtime);
    Ok(runtime_continuation_journal_snapshot_from_runtime(&runtime))
}

pub(crate) fn runtime_state_save_worker_loop(queue: Arc<RuntimeStateSaveQueue>) {
    runtime_run_scheduled_save_worker_loop(
        &queue.pending,
        &queue.wake,
        queue.active.as_ref(),
        |job| {
            let RuntimeStateSaveJob {
                payload,
                revision,
                latest_revision,
                log_path,
                reason,
                queued_at,
                ready_at: _,
            } = job;
            let result = match payload {
                RuntimeStateSavePayload::Snapshot(snapshot) => {
                    save_runtime_state_snapshot_if_latest(
                        &snapshot.paths,
                        &snapshot.state,
                        &snapshot.continuations,
                        &snapshot.profile_scores,
                        &snapshot.usage_snapshots,
                        &snapshot.backoffs,
                        revision,
                        &latest_revision,
                    )
                }
                RuntimeStateSavePayload::Live { shared, sections } => {
                    runtime_state_save_selected_snapshot_from_shared(&shared, sections).and_then(
                        |snapshot| {
                            save_runtime_state_selected_snapshot_if_latest(
                                &snapshot,
                                revision,
                                &latest_revision,
                            )
                        },
                    )
                }
            };
            match result {
                Ok(true) => runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "state_save_ok",
                        [
                            runtime_proxy_log_field("revision", revision.to_string()),
                            runtime_proxy_log_field("reason", reason.as_str()),
                            runtime_proxy_log_field(
                                "lag_ms",
                                queued_at.elapsed().as_millis().to_string(),
                            ),
                        ],
                    ),
                ),
                Ok(false) => runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "state_save_skipped",
                        [
                            runtime_proxy_log_field("revision", revision.to_string()),
                            runtime_proxy_log_field("reason", reason.as_str()),
                            runtime_proxy_log_field(
                                "lag_ms",
                                queued_at.elapsed().as_millis().to_string(),
                            ),
                        ],
                    ),
                ),
                Err(err) => runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "state_save_error",
                        [
                            runtime_proxy_log_field("revision", revision.to_string()),
                            runtime_proxy_log_field("reason", reason.as_str()),
                            runtime_proxy_log_field(
                                "lag_ms",
                                queued_at.elapsed().as_millis().to_string(),
                            ),
                            runtime_proxy_log_field("stage", "write"),
                            runtime_proxy_log_field("error", format!("{err:#}")),
                        ],
                    ),
                ),
            }
            runtime_allocator_trim_best_effort();
        },
    )
}

pub(crate) fn runtime_continuation_journal_save_worker_loop(
    queue: Arc<RuntimeContinuationJournalSaveQueue>,
) {
    runtime_run_scheduled_save_worker_loop(
        &queue.pending,
        &queue.wake,
        queue.active.as_ref(),
        |job| {
            let RuntimeContinuationJournalSaveJob {
                payload,
                log_path,
                reason,
                saved_at,
                queued_at,
                ready_at: _,
            } = job;
            let snapshot = match payload {
                RuntimeContinuationJournalSavePayload::Snapshot(snapshot) => Ok(snapshot),
                RuntimeContinuationJournalSavePayload::Live(shared) => {
                    runtime_continuation_journal_snapshot_from_shared(&shared)
                }
            };
            match snapshot.and_then(|snapshot| {
                save_runtime_continuation_journal_for_profiles(
                    &snapshot.paths,
                    &snapshot.continuations,
                    &snapshot.profiles,
                    saved_at,
                )
            }) {
                Ok(()) => runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "continuation_journal_save_ok",
                        [
                            runtime_proxy_log_field("saved_at", saved_at.to_string()),
                            runtime_proxy_log_field("reason", reason.as_str()),
                            runtime_proxy_log_field(
                                "lag_ms",
                                queued_at.elapsed().as_millis().to_string(),
                            ),
                        ],
                    ),
                ),
                Err(err) => runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "continuation_journal_save_error",
                        [
                            runtime_proxy_log_field("saved_at", saved_at.to_string()),
                            runtime_proxy_log_field("reason", reason.as_str()),
                            runtime_proxy_log_field(
                                "lag_ms",
                                queued_at.elapsed().as_millis().to_string(),
                            ),
                            runtime_proxy_log_field("stage", "write"),
                            runtime_proxy_log_field("error", format!("{err:#}")),
                        ],
                    ),
                ),
            }
            runtime_allocator_trim_best_effort();
        },
    )
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn malloc_trim(pad: usize) -> i32;
}

pub(crate) fn runtime_allocator_trim_best_effort() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    unsafe {
        let _ = malloc_trim(0);
    }
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

fn runtime_state_save_enqueue_plan(
    reason: &str,
    queued_at: Instant,
) -> prodex_runtime_state::RuntimeStateSaveEnqueuePlan {
    prodex_runtime_state::runtime_state_save_enqueue_plan(
        reason,
        queued_at,
        Duration::from_millis(RUNTIME_STATE_SAVE_DEBOUNCE_MS),
    )
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_state_save_reason_requires_continuation_journal(reason: &str) -> bool {
    prodex_runtime_state::runtime_state_save_reason_requires_continuation_journal(reason)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_state_save_sections_for_reason(reason: &str) -> RuntimeStateSaveSections {
    prodex_runtime_state::runtime_state_save_sections_for_reason(reason)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_state_save_debounce(reason: &str) -> Duration {
    prodex_runtime_state::runtime_state_save_debounce(
        reason,
        Duration::from_millis(RUNTIME_STATE_SAVE_DEBOUNCE_MS),
    )
}

fn runtime_continuation_journal_save_enqueue_plan(
    reason: &str,
    queued_at: Instant,
) -> prodex_runtime_state::RuntimeScheduledSaveEnqueuePlan {
    prodex_runtime_state::runtime_continuation_journal_save_enqueue_plan(
        reason,
        queued_at,
        Duration::from_millis(RUNTIME_STATE_SAVE_DEBOUNCE_MS),
    )
}
