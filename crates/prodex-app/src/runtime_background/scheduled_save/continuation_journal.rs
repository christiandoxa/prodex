use super::*;

pub(crate) fn schedule_runtime_continuation_journal_save_from_runtime(
    shared: &RuntimeRotationProxyShared,
    runtime: &RuntimeRotationState,
    mutation: &RuntimeStateMutation,
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
            mutation,
        );
        return;
    }
    let queue = runtime_continuation_journal_save_queue();
    let journal_path = runtime_continuation_journal_file_path(&runtime.paths);
    let reason = mutation.reason();
    let enqueue_plan = runtime_continuation_journal_save_enqueue_plan(mutation, Instant::now());
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
            reason: reason.clone(),
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
                runtime_proxy_log_field("reason", &reason),
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
                    runtime_proxy_log_field("reason", &reason),
                    runtime_proxy_log_field("backlog", backlog.to_string()),
                ],
            ),
        );
    }
}

pub(crate) fn schedule_runtime_continuation_journal_save(
    shared: &RuntimeRotationProxyShared,
    continuations: RuntimeContinuationStore,
    profiles: BTreeMap<String, ProfileEntry>,
    paths: AppPaths,
    mutation: &RuntimeStateMutation,
) {
    let reason = mutation.reason();
    if !runtime_proxy_persistence_enabled(shared) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "continuation_journal_save_suppressed",
                [
                    runtime_proxy_log_field("role", "follower"),
                    runtime_proxy_log_field("reason", &reason),
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
                    runtime_proxy_log_field("reason", &reason),
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
                        runtime_proxy_log_field("reason", &reason),
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
                        runtime_proxy_log_field("reason", &reason),
                        runtime_proxy_log_field("lag_ms", "0"),
                        runtime_proxy_log_field("stage", "write"),
                        runtime_proxy_log_field("error", runtime_scheduled_save_error(&err)),
                    ],
                ),
            ),
        }
        return;
    }
    let queue = runtime_continuation_journal_save_queue();
    let journal_path = runtime_continuation_journal_file_path(&paths);
    let enqueue_plan = runtime_continuation_journal_save_enqueue_plan(mutation, Instant::now());
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
            reason: reason.clone(),
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
                runtime_proxy_log_field("reason", &reason),
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
                    runtime_proxy_log_field("reason", &reason),
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
