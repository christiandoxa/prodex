use super::*;

pub(crate) fn runtime_probe_refresh_queue() -> Arc<RuntimeProbeRefreshQueue> {
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

#[allow(dead_code)]
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

pub(crate) fn runtime_profiles_needing_startup_probe_refresh(
    state: &AppState,
    current_profile: &str,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    profile_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
) -> Vec<String> {
    prodex_runtime_state::runtime_profiles_needing_startup_probe_refresh_from_snapshots(
        active_profile_selection_order(state, current_profile)
            .into_iter()
            .map(
                |profile_name| prodex_runtime_state::RuntimeStartupProbeRefreshInput {
                    probe_checked_at: profile_probe_cache
                        .get(&profile_name)
                        .map(|entry| entry.checked_at),
                    usage_snapshot: profile_usage_snapshots.get(&profile_name),
                    profile_name,
                },
            ),
        prodex_runtime_state::RuntimeStartupProbeRefreshPlan {
            now,
            probe_fresh_seconds: RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS,
            stale_grace_seconds: RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
            warm_limit: RUNTIME_STARTUP_PROBE_WARM_LIMIT,
        },
        |status| matches!(status, RuntimeQuotaWindowStatus::Exhausted),
    )
}

pub(crate) fn run_runtime_probe_jobs_inline(
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
    let upstream_no_proxy = shared.upstream_no_proxy;
    let probe_reports = map_parallel(jobs, |(profile_name, codex_home)| {
        (
            profile_name,
            RuntimeProbeRefreshAttempt::collect(
                &codex_home,
                upstream_base_url.as_str(),
                upstream_no_proxy,
            ),
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

pub(crate) fn schedule_runtime_startup_probe_warmup(shared: &RuntimeRotationProxyShared) {
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

fn runtime_probe_refresh_take_next_job(queue: &RuntimeProbeRefreshQueue) -> RuntimeProbeRefreshJob {
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loop {
        if let Some(key) = pending.keys().next().cloned() {
            return pending
                .remove(&key)
                .expect("selected probe refresh job should remain queued");
        }
        pending = queue
            .wake
            .wait(pending)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

pub(crate) fn runtime_probe_refresh_worker_loop(queue: Arc<RuntimeProbeRefreshQueue>) {
    loop {
        let job = runtime_probe_refresh_take_next_job(&queue);
        queue.active.fetch_add(1, Ordering::SeqCst);
        let log_path = job.shared.log_path.clone();
        let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job.execute()));
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
                &runtime_proxy_structured_log_message(
                    "profile_probe_refresh_panic",
                    [runtime_proxy_log_field("error", panic_message)],
                ),
            );
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
                            runtime_proxy_structured_log_message(
                                "profile_probe_refresh_error",
                                [
                                    runtime_proxy_log_field("profile", profile_name),
                                    runtime_proxy_log_field("lag_ms", lag_ms.to_string()),
                                    runtime_proxy_log_field(
                                        "error",
                                        format!("state_update:{err:#}"),
                                    ),
                                ],
                            )
                        } else {
                            runtime_proxy_structured_log_message(
                                "profile_probe_refresh_ok",
                                [
                                    runtime_proxy_log_field("profile", profile_name),
                                    runtime_proxy_log_field("lag_ms", lag_ms.to_string()),
                                ],
                            )
                        },
                    ),
                    Err(err) => runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "profile_probe_refresh_error",
                            [
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field("lag_ms", lag_ms.to_string()),
                                runtime_proxy_log_field("error", err.as_str()),
                            ],
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
    fn collect(codex_home: &Path, upstream_base_url: &str, upstream_no_proxy: bool) -> Self {
        let auth = read_auth_summary(codex_home);
        let result = if auth.quota_compatible {
            fetch_usage_with_proxy_policy(codex_home, Some(upstream_base_url), upstream_no_proxy)
                .map_err(|err| err.to_string())
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
pub(crate) fn execute_runtime_probe_attempt_inline_for_test(
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
pub(crate) fn execute_runtime_probe_attempt_queued_for_test(
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

trait RuntimeProbeRefreshJobExt {
    fn execute(self);
}

impl RuntimeProbeRefreshJobExt for RuntimeProbeRefreshJob {
    fn execute(self) {
        runtime_proxy_log(
            &self.shared,
            runtime_proxy_structured_log_message(
                "profile_probe_refresh_start",
                [runtime_proxy_log_field(
                    "profile",
                    self.profile_name.as_str(),
                )],
            ),
        );
        RuntimeProbeRefreshAttempt::collect(
            &self.codex_home,
            self.upstream_base_url.as_str(),
            self.shared.upstream_no_proxy,
        )
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

#[cfg(test)]
#[path = "../../tests/src/runtime_background/probe_refresh.rs"]
mod tests;
