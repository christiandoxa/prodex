use super::super::*;

pub(crate) fn apply_runtime_profile_probe_result(
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
            runtime_quota_summary_blocking_reset_at(quota_summary, RuntimeRouteKind::Responses);
        let usage_apply_plan = prodex_runtime_state::runtime_probe_usage_snapshot_apply_plan(
            prodex_runtime_state::RuntimeProbeUsageSnapshotApplyInput {
                previous_snapshot: previous_snapshot.as_ref(),
                previous_retry_backoff_until: previous_retry_backoff,
                next_snapshot: &snapshot,
                quota_blocked: runtime_quota_precommit_guard_reason(
                    quota_summary,
                    RuntimeRouteKind::Responses,
                )
                .is_some(),
                blocking_reset_at,
                now,
                quota_quarantine_fallback_seconds:
                    RUNTIME_PROFILE_QUOTA_QUARANTINE_FALLBACK_SECONDS,
                touch_persist_interval_seconds: RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS,
            },
        );
        if let Some(until) = usage_apply_plan.retry_backoff_until {
            runtime
                .profile_retry_backoff_until
                .insert(profile_name.to_string(), until);
            mark_runtime_profile_retry_backoff_update(runtime, profile_name);
        }
        runtime
            .profile_usage_snapshots
            .insert(profile_name.to_string(), snapshot);
        if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
            log_messages.push(format!(
                "quota_probe_exhausted profile={profile_name} reason=usage_snapshot_exhausted {}",
                runtime_quota_summary_log_fields(quota_summary)
            ));
        }
        if let Some(until) = usage_apply_plan.retry_backoff_until
            && usage_apply_plan.retry_backoff_changed
        {
            log_messages.push(format!(
                "profile_quota_quarantine profile={profile_name} route={} until={} reset_at={} message=probe_snapshot",
                runtime_route_kind_label(RuntimeRouteKind::Responses),
                until,
                usage_apply_plan.blocking_reset_at.unwrap_or(i64::MAX),
            ));
            log_messages.push(format!(
                "profile_retry_backoff profile={profile_name} until={until}",
            ));
        }
        if usage_apply_plan.snapshot_should_persist || usage_apply_plan.retry_backoff_changed {
            state_save_args = Some(RuntimeStateSaveRequest::from_snapshot(
                runtime_state_save_snapshot_from_runtime(runtime),
                RuntimeStateMutation::UsageSnapshot(profile_name.to_string()),
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

pub(super) fn runtime_probe_refresh_apply_wait_timeout() -> Duration {
    Duration::from_millis(if cfg!(test) { 250 } else { 1_000 })
}

pub(super) fn apply_runtime_profile_probe_result_with_timeout(
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

struct RuntimeProbeProgressObserver;

impl RuntimeProbeProgressObserver {
    fn new() -> Self {
        Self
    }
}

impl Drop for RuntimeProbeProgressObserver {
    fn drop(&mut self) {
        super::queue::note_runtime_probe_refresh_progress();
    }
}
