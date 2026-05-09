use super::*;

pub(crate) fn schedule_runtime_smart_context_artifact_save(
    shared: &RuntimeRotationProxyShared,
    path: PathBuf,
    store: RuntimeSmartContextArtifactStore,
    reason: &str,
) {
    if !runtime_proxy_persistence_enabled(shared) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "smart_context_artifact_save_suppressed",
                [
                    runtime_proxy_log_field("role", "follower"),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field("path", path.display().to_string()),
                ],
            ),
        );
        return;
    }

    if cfg!(test) {
        match store.save_merged_to_path(&path) {
            Ok(merged) => runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "smart_context_artifact_save_ok",
                    [
                        runtime_proxy_log_field("reason", reason),
                        runtime_proxy_log_field("lag_ms", "0"),
                        runtime_proxy_log_field("artifacts", merged.artifact_count().to_string()),
                    ],
                ),
            ),
            Err(err) => runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "smart_context_artifact_save_error",
                    [
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

    let queue = runtime_smart_context_artifact_save_queue();
    let queued_at = Instant::now();
    let ready_at = queued_at + Duration::from_millis(RUNTIME_SMART_CONTEXT_ARTIFACT_SAVE_DELAY_MS);
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pending.insert(
        path.clone(),
        RuntimeSmartContextArtifactSaveJob {
            path,
            store,
            log_path: shared.log_path.clone(),
            reason: reason.to_string(),
            queued_at,
            ready_at,
        },
    );
    let backlog = pending.len();
    drop(pending);
    queue.wake.notify_one();

    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "smart_context_artifact_save_queued",
            [
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("backlog", backlog.to_string()),
                runtime_proxy_log_field(
                    "ready_in_ms",
                    RUNTIME_SMART_CONTEXT_ARTIFACT_SAVE_DELAY_MS.to_string(),
                ),
            ],
        ),
    );
}

pub(crate) fn runtime_smart_context_artifact_save_queue()
-> Arc<RuntimeSmartContextArtifactSaveQueue> {
    Arc::clone(RUNTIME_SMART_CONTEXT_ARTIFACT_SAVE_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeSmartContextArtifactSaveQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
            active: Arc::new(AtomicUsize::new(0)),
        });
        let worker_queue = Arc::clone(&queue);
        thread::spawn(move || runtime_smart_context_artifact_save_worker_loop(worker_queue));
        queue
    }))
}
