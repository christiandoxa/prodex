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
    let backlog = pending.len().saturating_sub(1);
    drop(pending);
    queue.wake.notify_one();
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
    if runtime_proxy_queue_pressure_active(0, 0, backlog) {
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

pub(crate) fn runtime_profile_usage_snapshot_materially_matches(
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

pub(crate) fn runtime_profile_usage_snapshot_should_persist(
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

impl RuntimeProbeRefreshJob {
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
mod tests {
    use super::*;

    fn test_runtime_probe_refresh_queue() -> RuntimeProbeRefreshQueue {
        RuntimeProbeRefreshQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
            active: Arc::new(AtomicUsize::new(0)),
            wait: Arc::new((Mutex::new(()), Condvar::new())),
            revision: Arc::new(AtomicU64::new(0)),
        }
    }

    fn test_runtime_probe_refresh_shared() -> RuntimeRotationProxyShared {
        let root =
            std::env::temp_dir().join(format!("prodex-probe-refresh-unit-{}", std::process::id()));
        RuntimeRotationProxyShared {
            upstream_no_proxy: false,
            async_client: reqwest::Client::builder().build().expect("async client"),
            async_runtime: Arc::new(
                TokioRuntimeBuilder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("async runtime"),
            ),
            runtime: Arc::new(Mutex::new(RuntimeRotationState {
                paths: AppPaths {
                    root: root.join("prodex"),
                    state_file: root.join("prodex/state.json"),
                    managed_profiles_root: root.join("prodex/profiles"),
                    shared_codex_root: root.join("shared"),
                    legacy_shared_codex_root: root.join("prodex/shared"),
                },
                state: AppState::default(),
                upstream_base_url: "http://127.0.0.1:1/backend-api".to_string(),
                include_code_review: false,
                current_profile: "main".to_string(),
                profile_usage_auth: BTreeMap::new(),
                turn_state_bindings: BTreeMap::new(),
                session_id_bindings: BTreeMap::new(),
                continuation_statuses: RuntimeContinuationStatuses::default(),
                profile_probe_cache: BTreeMap::new(),
                profile_usage_snapshots: BTreeMap::new(),
                profile_retry_backoff_until: BTreeMap::new(),
                profile_transport_backoff_until: BTreeMap::new(),
                profile_route_circuit_open_until: BTreeMap::new(),
                profile_inflight: BTreeMap::new(),
                profile_health: BTreeMap::new(),
            })),
            log_path: root.join("runtime-proxy.log"),
            request_sequence: Arc::new(AtomicU64::new(1)),
            state_save_revision: Arc::new(AtomicU64::new(0)),
            local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
            active_request_count: Arc::new(AtomicUsize::new(0)),
            active_request_limit: 1,
            runtime_state_lock_wait_counters:
                RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
            lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
                responses: 1,
                compact: 1,
                websocket: 1,
                standard: 1,
            }),
        }
    }

    fn test_runtime_probe_refresh_job(
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
    ) -> RuntimeProbeRefreshJob {
        RuntimeProbeRefreshJob {
            shared: shared.clone(),
            profile_name: profile_name.to_string(),
            codex_home: PathBuf::from(format!("/tmp/{profile_name}")),
            upstream_base_url: "http://127.0.0.1:1/backend-api".to_string(),
            queued_at: Instant::now(),
        }
    }

    #[test]
    fn runtime_probe_refresh_take_next_job_leaves_remaining_backlog_for_other_workers() {
        let shared = test_runtime_probe_refresh_shared();
        let queue = test_runtime_probe_refresh_queue();
        {
            let mut pending = queue
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            pending.insert(
                (PathBuf::from("/tmp/state.json"), "alpha".to_string()),
                test_runtime_probe_refresh_job(&shared, "alpha"),
            );
            pending.insert(
                (PathBuf::from("/tmp/state.json"), "beta".to_string()),
                test_runtime_probe_refresh_job(&shared, "beta"),
            );
        }

        let first = runtime_probe_refresh_take_next_job(&queue);
        assert_eq!(first.profile_name, "alpha");
        assert_eq!(
            queue
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len(),
            1,
            "one dequeue should leave the rest of the backlog available to other workers"
        );

        let second = runtime_probe_refresh_take_next_job(&queue);
        assert_eq!(second.profile_name, "beta");
        assert!(
            queue
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_empty(),
            "second worker should be able to claim the remaining queued job"
        );
    }
}
