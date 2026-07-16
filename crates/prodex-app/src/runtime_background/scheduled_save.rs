use super::*;
use redaction::redaction_redact_secret_like_text;

const RUNTIME_SMART_CONTEXT_ARTIFACT_SAVE_DELAY_MS: u64 = if cfg!(test) { 0 } else { 25 };

mod artifact_save;
mod continuation_journal;
mod queues;
mod workers;

pub(crate) use artifact_save::*;
pub(crate) use continuation_journal::*;
pub(crate) use queues::*;
pub(crate) use workers::*;

pub(crate) struct RuntimeStateSaveRequest {
    state: AppState,
    continuations: RuntimeContinuationStore,
    profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: RuntimeProfileBackoffs,
    paths: AppPaths,
    mutation: RuntimeStateMutation,
}

impl RuntimeStateSaveRequest {
    pub(crate) fn from_snapshot(
        snapshot: RuntimeStateSaveSnapshot,
        mutation: RuntimeStateMutation,
    ) -> Self {
        Self {
            state: snapshot.state,
            continuations: snapshot.continuations,
            profile_scores: snapshot.profile_scores,
            usage_snapshots: snapshot.usage_snapshots,
            backoffs: snapshot.backoffs,
            paths: snapshot.paths,
            mutation,
        }
    }
}

pub(crate) fn schedule_runtime_state_save_request(
    shared: &RuntimeRotationProxyShared,
    request: RuntimeStateSaveRequest,
) {
    let reason = request.mutation.reason();
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
    let enqueue_plan = runtime_state_save_enqueue_plan(&request.mutation, Instant::now());
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
        match save_runtime_state_snapshot_if_latest(RuntimeStateSnapshotSaveInput {
            paths: &request.paths,
            snapshot: &request.state,
            continuations: &request.continuations,
            profile_scores: &request.profile_scores,
            usage_snapshots: &request.usage_snapshots,
            backoffs: &request.backoffs,
            revision,
            latest_revision: &shared.state_save_revision,
        }) {
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
                        runtime_proxy_log_field("error", runtime_scheduled_save_error(&err)),
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
                &request.mutation,
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
    if let Some(continuations) = journal_continuations {
        schedule_runtime_continuation_journal_save(
            shared,
            continuations,
            state_profiles,
            request.paths,
            &request.mutation,
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
    mutation: RuntimeStateMutation,
) {
    if !runtime_proxy_persistence_enabled(shared) {
        return;
    }
    if cfg!(test) {
        schedule_runtime_state_save_request(
            shared,
            RuntimeStateSaveRequest::from_snapshot(
                runtime_state_save_snapshot_from_runtime(runtime),
                mutation,
            ),
        );
        return;
    }
    let revision = shared.state_save_revision.fetch_add(1, Ordering::SeqCst) + 1;
    let reason = mutation.reason();
    let enqueue_plan = runtime_state_save_enqueue_plan(&mutation, Instant::now());
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
        &reason,
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
                runtime_proxy_log_field("reason", &reason),
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
                    runtime_proxy_log_field("reason", &reason),
                    runtime_proxy_log_field("backlog", backlog.to_string()),
                ],
            ),
        );
    }
    if plan.requires_continuation_journal {
        schedule_runtime_continuation_journal_save_from_runtime(shared, runtime, &mutation);
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

fn runtime_state_save_enqueue_plan(
    mutation: &RuntimeStateMutation,
    queued_at: Instant,
) -> prodex_runtime_state::RuntimeStateSaveEnqueuePlan {
    prodex_runtime_state::runtime_state_save_enqueue_plan(
        mutation,
        queued_at,
        Duration::from_millis(RUNTIME_STATE_SAVE_DEBOUNCE_MS),
    )
}

#[cfg(test)]
pub(crate) fn runtime_state_save_requires_continuation_journal(
    mutation: &RuntimeStateMutation,
) -> bool {
    prodex_runtime_state::runtime_state_save_requires_continuation_journal(mutation)
}

#[cfg(test)]
pub(crate) fn runtime_state_save_sections(
    mutation: &RuntimeStateMutation,
) -> RuntimeStateSaveSections {
    prodex_runtime_state::runtime_state_save_sections(mutation)
}

#[cfg(test)]
pub(crate) fn runtime_state_save_debounce(mutation: &RuntimeStateMutation) -> Duration {
    prodex_runtime_state::runtime_state_save_debounce(
        mutation,
        Duration::from_millis(RUNTIME_STATE_SAVE_DEBOUNCE_MS),
    )
}

fn runtime_continuation_journal_save_enqueue_plan(
    mutation: &RuntimeStateMutation,
    queued_at: Instant,
) -> prodex_runtime_state::RuntimeScheduledSaveEnqueuePlan {
    prodex_runtime_state::runtime_continuation_journal_save_enqueue_plan(
        mutation,
        queued_at,
        Duration::from_millis(RUNTIME_STATE_SAVE_DEBOUNCE_MS),
    )
}

fn runtime_scheduled_save_error(err: &anyhow::Error) -> String {
    redaction_redact_secret_like_text(&format!("{err:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_scheduled_save_error_redacts_secret_like_chain() {
        let err = anyhow::anyhow!("failed: Authorization: Bearer scheduled-save-token")
            .context("state save failed");

        let message = runtime_scheduled_save_error(&err);

        assert!(message.contains("state save failed"));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(!message.contains("scheduled-save-token"));
    }
}
