use super::*;

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
                    save_runtime_state_snapshot_if_latest(RuntimeStateSnapshotSaveInput {
                        paths: &snapshot.paths,
                        snapshot: &snapshot.state,
                        continuations: &snapshot.continuations,
                        profile_scores: &snapshot.profile_scores,
                        usage_snapshots: &snapshot.usage_snapshots,
                        backoffs: &snapshot.backoffs,
                        revision,
                        latest_revision: &latest_revision,
                    })
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
                            runtime_proxy_log_field("error", runtime_scheduled_save_error(&err)),
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
                            runtime_proxy_log_field("error", runtime_scheduled_save_error(&err)),
                        ],
                    ),
                ),
            }
            runtime_allocator_trim_best_effort();
        },
    )
}

pub(crate) fn runtime_smart_context_artifact_save_worker_loop(
    queue: Arc<RuntimeSmartContextArtifactSaveQueue>,
) {
    runtime_run_scheduled_save_worker_loop(
        &queue.pending,
        &queue.wake,
        queue.active.as_ref(),
        |job| {
            let RuntimeSmartContextArtifactSaveJob {
                path,
                store,
                log_path,
                reason,
                queued_at,
                ready_at: _,
            } = job;
            match store.save_merged_to_path(&path) {
                Ok(merged) => runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "smart_context_artifact_save_ok",
                        [
                            runtime_proxy_log_field("reason", reason.as_str()),
                            runtime_proxy_log_field(
                                "lag_ms",
                                queued_at.elapsed().as_millis().to_string(),
                            ),
                            runtime_proxy_log_field(
                                "artifacts",
                                merged.artifact_count().to_string(),
                            ),
                        ],
                    ),
                ),
                Err(err) => runtime_proxy_log_to_path(
                    &log_path,
                    &runtime_proxy_structured_log_message(
                        "smart_context_artifact_save_error",
                        [
                            runtime_proxy_log_field("reason", reason.as_str()),
                            runtime_proxy_log_field(
                                "lag_ms",
                                queued_at.elapsed().as_millis().to_string(),
                            ),
                            runtime_proxy_log_field("stage", "write"),
                            runtime_proxy_log_field("error", runtime_scheduled_save_error(&err)),
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
