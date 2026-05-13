use super::super::*;
use super::worker::runtime_probe_refresh_worker_loop;
use crate::runtime_background::worker_spawn::spawn_runtime_background_worker_or_panic;

pub(crate) fn runtime_probe_refresh_queue() -> Arc<RuntimeProbeRefreshQueue> {
    Arc::clone(RUNTIME_PROBE_REFRESH_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeProbeRefreshQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
            active: Arc::new(AtomicUsize::new(0)),
            wait: Arc::new((Mutex::new(()), Condvar::new())),
            revision: Arc::new(AtomicU64::new(0)),
        });
        for worker_index in 0..runtime_probe_refresh_worker_count() {
            let worker_queue = Arc::clone(&queue);
            spawn_runtime_background_worker_or_panic(
                format!("prodex-runtime-probe-refresh-{worker_index}"),
                None,
                move || runtime_probe_refresh_worker_loop(worker_queue),
            );
        }
        queue
    }))
}

pub(crate) fn runtime_probe_refresh_queue_backlog() -> usize {
    runtime_probe_refresh_queue()
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

pub(crate) fn runtime_probe_refresh_revision() -> u64 {
    runtime_probe_refresh_queue()
        .revision
        .load(Ordering::SeqCst)
}

pub(crate) fn note_runtime_probe_refresh_progress() {
    let queue = runtime_probe_refresh_queue();
    queue.revision.fetch_add(1, Ordering::SeqCst);
    let (mutex, condvar) = &*queue.wait;
    let _guard = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    condvar.notify_all();
}

#[cfg(test)]
pub(crate) fn runtime_probe_refresh_queue_active() -> usize {
    runtime_probe_refresh_queue().active.load(Ordering::SeqCst)
}

pub(crate) fn schedule_runtime_probe_refresh(
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
            runtime_proxy_structured_log_message(
                "profile_probe_refresh_suppressed",
                [
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("reason", "test_nonlocal_upstream"),
                ],
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
    let queue_plan = runtime_background_queue_enqueue_plan(
        prodex_runtime_state::RuntimeBackgroundQueueKind::ProbeRefresh,
        pending.len(),
    );
    drop(pending);
    queue.wake.notify_one();
    let backlog = queue_plan.backlog;
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_probe_refresh_queued",
            [
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("reason", "queued"),
                runtime_proxy_log_field("backlog", backlog.to_string()),
            ],
        ),
    );
    if queue_plan.pressure_active {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "profile_probe_refresh_backpressure",
                [
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("backlog", backlog.to_string()),
                ],
            ),
        );
    }
}

#[cfg(test)]
pub(crate) fn runtime_probe_refresh_nonlocal_upstream_for_test(upstream_base_url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(upstream_base_url) else {
        return true;
    };
    !matches!(
        url.host_str(),
        Some("127.0.0.1") | Some("localhost") | Some("::1") | Some("[::1]")
    )
}
