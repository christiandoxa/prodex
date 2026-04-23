use super::*;

pub(super) fn running_in_ci() -> bool {
    std::env::var_os("GITHUB_ACTIONS").is_some() || std::env::var_os("CI").is_some()
}

pub(super) fn ci_timing_upper_bound_ms(local_ms: u64, ci_ms: u64) -> Duration {
    if running_in_ci() {
        Duration::from_millis(ci_ms.max(local_ms))
    } else {
        Duration::from_millis(local_ms)
    }
}

pub(super) fn ci_timing_budget_ms(local_ms: u64, ci_ms: u64) -> String {
    if running_in_ci() {
        ci_ms.max(local_ms).to_string()
    } else {
        local_ms.to_string()
    }
}

pub(super) fn ci_runtime_proxy_timeout_guard(
    env_key: &'static str,
    local_ms: u64,
    ci_ms: u64,
) -> TestEnvVarGuard {
    TestEnvVarGuard::set(env_key, &ci_timing_budget_ms(local_ms, ci_ms))
}

pub(super) fn ci_runtime_proxy_admission_wait_budget_guard(local_ms: u64, ci_ms: u64) -> TestEnvVarGuard {
    ci_runtime_proxy_timeout_guard(
        "PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS",
        local_ms,
        ci_ms,
    )
}

pub(super) fn ci_runtime_proxy_websocket_timeout_guards() -> (TestEnvVarGuard, TestEnvVarGuard) {
    (
        ci_runtime_proxy_timeout_guard(
            "PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS",
            250,
            1_000,
        ),
        ci_runtime_proxy_timeout_guard(
            "PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS",
            120,
            1_000,
        ),
    )
}

pub(super) fn usage_with_main_windows(
    five_hour_remaining: i64,
    five_hour_reset_offset_seconds: i64,
    weekly_remaining: i64,
    weekly_reset_offset_seconds: i64,
) -> UsageResponse {
    let now = Local::now().timestamp();
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some((100 - five_hour_remaining).clamp(0, 100)),
                reset_at: Some(now + five_hour_reset_offset_seconds),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some((100 - weekly_remaining).clamp(0, 100)),
                reset_at: Some(now + weekly_reset_offset_seconds),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

pub(super) struct TestDir {
    pub(super) path: PathBuf,
}

static TEST_DIR_SEQUENCE: AtomicU64 = AtomicU64::new(1);

impl TestDir {
    pub(super) fn new() -> Self {
        wait_for_runtime_background_queues_idle();
        for _ in 0..32 {
            let unique = format!(
                "prodex-runtime-test-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock should be after unix epoch")
                    .as_nanos(),
                TEST_DIR_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            );
            let path = std::env::temp_dir().join(unique);
            match fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => panic!("failed to create test temp dir: {err}"),
            }
        }
        panic!("failed to allocate unique test temp dir after repeated collisions");
    }
}

pub(super) fn wait_for_runtime_background_queues_idle() {
    let deadline = Instant::now() + ci_timing_upper_bound_ms(5_000, 10_000);
    let probe_refresh_grace = ci_timing_upper_bound_ms(250, 2_000);
    let mut lingering_probe_refresh_since = None;
    loop {
        let state_save_backlog = runtime_state_save_queue_backlog();
        let state_save_active = runtime_state_save_queue_active();
        let continuation_backlog = runtime_continuation_journal_queue_backlog();
        let continuation_active = runtime_continuation_journal_queue_active();
        let probe_refresh_backlog = runtime_probe_refresh_queue_backlog();
        let probe_refresh_active = runtime_probe_refresh_queue_active();
        let backlog = state_save_backlog + continuation_backlog + probe_refresh_backlog;
        let active = state_save_active + continuation_active + probe_refresh_active;
        if backlog == 0 && active == 0 {
            return;
        }
        let only_lingering_probe_refresh = state_save_backlog == 0
            && state_save_active == 0
            && continuation_backlog == 0
            && continuation_active == 0
            && (probe_refresh_backlog > 0 || probe_refresh_active > 0);
        if only_lingering_probe_refresh {
            let lingering_since = lingering_probe_refresh_since.get_or_insert_with(Instant::now);
            // Tests use isolated state roots, so once every other queue has drained we can
            // discard stale best-effort probe refresh backlog instead of timing out on work
            // that belongs to a previous test. Any workers already in flight can finish in the
            // background without touching the next test's state root.
            if lingering_since.elapsed() >= probe_refresh_grace {
                if probe_refresh_backlog > 0 {
                    let queue = runtime_probe_refresh_queue();
                    let mut pending = queue
                        .pending
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    pending.clear();
                }
                return;
            }
        } else {
            lingering_probe_refresh_since = None;
        }
        if Instant::now() >= deadline {
            panic!(
                "runtime background queues did not go idle before timeout: backlog={backlog} active={active} state_save_backlog={state_save_backlog} state_save_active={state_save_active} continuation_backlog={continuation_backlog} continuation_active={continuation_active} probe_refresh_backlog={probe_refresh_backlog} probe_refresh_active={probe_refresh_active}"
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn stale_critical_runtime_usage_snapshot(now: i64) -> RuntimeProfileUsageSnapshot {
    RuntimeProfileUsageSnapshot {
        checked_at: now - (RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS + 1),
        five_hour_status: RuntimeQuotaWindowStatus::Critical,
        five_hour_remaining_percent: 1,
        five_hour_reset_at: now + 300,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 70,
        weekly_reset_at: now + 86_400,
    }
}

pub(super) fn ready_runtime_usage_snapshot(now: i64, remaining_percent: i64) -> RuntimeProfileUsageSnapshot {
    RuntimeProfileUsageSnapshot {
        checked_at: now,
        five_hour_status: RuntimeQuotaWindowStatus::Ready,
        five_hour_remaining_percent: remaining_percent,
        five_hour_reset_at: now + 18_000,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: remaining_percent,
        weekly_reset_at: now + 604_800,
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(super) fn runtime_rotation_proxy_shared(
    temp_dir: &TestDir,
    runtime: RuntimeRotationState,
    active_request_limit: usize,
) -> RuntimeRotationProxyShared {
    let active_request_limit = active_request_limit.max(1);
    RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        log_path: temp_dir.path.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(active_request_limit),
        runtime: Arc::new(Mutex::new(runtime)),
    }
}

pub(super) fn runtime_shared_for_cold_start_probe_selection(
    temp_dir: &TestDir,
    upstream_base_url: String,
) -> RuntimeRotationProxyShared {
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    runtime_rotation_proxy_shared(
        temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([
                    (
                        "main".to_string(),
                        ProfileEntry {
                            codex_home: main_home,
                            managed: true,
                            email: Some("main@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                    (
                        "second".to_string(),
                        ProfileEntry {
                            codex_home: second_home,
                            managed: true,
                            email: Some("second@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                ]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url,
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(80, 300, 80, 86_400)),
                },
            )]),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::from([(
                runtime_profile_auth_failure_key("main"),
                RuntimeProfileHealth {
                    score: 1,
                    updated_at: now,
                },
            )]),
        },
        usize::MAX,
    )
}

pub(super) struct TestProbeRefreshBacklogGuard {
    keys: Vec<(PathBuf, String)>,
}

impl Drop for TestProbeRefreshBacklogGuard {
    fn drop(&mut self) {
        let queue = runtime_probe_refresh_queue();
        let mut pending = queue
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for key in &self.keys {
            pending.remove(key);
        }
    }
}

pub(super) fn force_runtime_probe_refresh_backlog(
    shared: &RuntimeRotationProxyShared,
    backlog: usize,
) -> TestProbeRefreshBacklogGuard {
    wait_for_runtime_background_queues_idle();
    let (state_file, upstream_base_url, root) = {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        (
            runtime.paths.state_file.clone(),
            runtime.upstream_base_url.clone(),
            runtime.paths.root.clone(),
        )
    };
    let queue = runtime_probe_refresh_queue();
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(
        pending.is_empty(),
        "probe refresh backlog helper requires an idle queue"
    );
    let mut keys = Vec::with_capacity(backlog);
    for index in 0..backlog {
        let profile_name = format!("probe-pressure-{index}");
        let key = (state_file.clone(), profile_name.clone());
        pending.insert(
            key.clone(),
            RuntimeProbeRefreshJob {
                shared: shared.clone(),
                profile_name,
                codex_home: root.join(format!("probe-pressure-{index}")),
                upstream_base_url: upstream_base_url.clone(),
                queued_at: Instant::now(),
            },
        );
        keys.push(key);
    }
    TestProbeRefreshBacklogGuard { keys }
}

pub(super) fn wait_for_runtime_log_tail_until<F, G>(
    mut read_tail: G,
    predicate: F,
    local_ms: u64,
    ci_ms: u64,
    poll_ms: u64,
) -> Vec<u8>
where
    F: Fn(&str) -> bool,
    G: FnMut() -> Option<Vec<u8>>,
{
    let deadline = Instant::now() + ci_timing_upper_bound_ms(local_ms, ci_ms);
    let mut last_tail = Vec::new();

    loop {
        if let Some(tail) = read_tail() {
            last_tail = tail;
            let text = String::from_utf8_lossy(&last_tail);
            if predicate(&text) {
                return last_tail;
            }
        }

        if Instant::now() >= deadline {
            return last_tail;
        }

        thread::sleep(Duration::from_millis(poll_ms));
    }
}

pub(super) fn wait_for_state<F>(paths: &AppPaths, predicate: F) -> AppState
where
    F: Fn(&AppState) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_state = None;
    loop {
        if let Ok(state) = AppState::load(paths) {
            if predicate(&state) {
                return state;
            }
            last_state = Some(state);
        }
        if Instant::now() >= deadline {
            let state = AppState::load(paths).expect("state should reload");
            panic!(
                "timed out waiting for app state predicate; last_state={:?} final_state={:?}",
                last_state, state
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}


pub(super) fn write_versioned_runtime_sidecar<T: Serialize>(
    path: &Path,
    backup_path: &Path,
    generation: u64,
    value: &T,
) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("versioned sidecar primary dir should exist");
    }
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).expect("versioned sidecar backup dir should exist");
    }
    let json = serde_json::to_string_pretty(&VersionedJson { generation, value })
        .expect("versioned sidecar should serialize");
    fs::write(path, &json).expect("versioned sidecar primary should write");
    fs::write(backup_path, &json).expect("versioned sidecar backup should write");
}

pub(super) fn tiny_http_response_status_and_body(response: tiny_http::ResponseBox) -> (u16, String) {
    let status = response.status_code().0;
    let mut bytes = Vec::new();
    response
        .raw_print(&mut bytes, (1, 0).into(), &[], false, None)
        .expect("response should serialize");
    let text = String::from_utf8(bytes).expect("response bytes should be utf8");
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    (status, body)
}

pub(super) fn tiny_http_response_status_content_type_and_body(
    response: tiny_http::ResponseBox,
) -> (u16, Option<String>, String) {
    let status = response.status_code().0;
    let mut bytes = Vec::new();
    response
        .raw_print(&mut bytes, (1, 0).into(), &[], false, None)
        .expect("response should serialize");
    let text = String::from_utf8(bytes).expect("response bytes should be utf8");
    let (headers, body) = text.split_once("\r\n\r\n").unwrap_or((text.as_str(), ""));
    let content_type = headers.lines().find_map(|line| {
        line.strip_prefix("Content-Type: ")
            .map(|value| value.to_string())
    });
    (status, content_type, body.to_string())
}
