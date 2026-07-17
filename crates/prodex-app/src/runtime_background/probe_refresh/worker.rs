use super::super::*;
use super::apply::runtime_probe_refresh_apply_wait_timeout;
use super::attempt::{RuntimeProbeExecutionMode, RuntimeProbeRefreshAttempt};

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_probe_refresh_take_next_job(
    queue: &RuntimeProbeRefreshQueue,
) -> RuntimeProbeRefreshJob {
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loop {
        if let Some(key) = pending.keys().next().cloned() {
            if let Some(job) = pending.remove(&key) {
                return job;
            }
            continue;
        }
        pending = queue
            .wake
            .wait(pending)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

pub(super) fn runtime_probe_refresh_worker_loop(queue: Arc<RuntimeProbeRefreshQueue>) {
    loop {
        let job = runtime_probe_refresh_take_next_job(&queue);
        queue.active.fetch_add(1, Ordering::SeqCst);
        let log_path = job.shared.log_path.clone();
        let panic_result = crate::runtime_panic::catch_runtime_unwind_silently(|| {
            execute_runtime_probe_refresh_job(job)
        });
        queue.active.fetch_sub(1, Ordering::SeqCst);
        if let Err(panic_payload) = panic_result {
            let panic_message =
                crate::runtime_panic::runtime_panic_payload_label(panic_payload.as_ref());
            runtime_proxy_log_to_path(
                &log_path,
                &runtime_proxy_structured_log_message(
                    "profile_probe_refresh_panic",
                    [runtime_proxy_log_field("error", panic_message)],
                ),
            );
        }
    }
}

fn execute_runtime_probe_refresh_job(job: RuntimeProbeRefreshJob) {
    runtime_proxy_log(
        &job.shared,
        runtime_proxy_structured_log_message(
            "profile_probe_refresh_start",
            [runtime_proxy_log_field(
                "profile",
                job.profile_name.as_str(),
            )],
        ),
    );
    RuntimeProbeRefreshAttempt::collect(
        &job.codex_home,
        job.upstream_base_url.as_str(),
        job.shared.upstream_no_proxy,
    )
    .execute(
        RuntimeProbeExecutionMode::Queued {
            apply_timeout: runtime_probe_refresh_apply_wait_timeout(),
        },
        &job.shared,
        &job.profile_name,
        job.queued_at,
    );
}
