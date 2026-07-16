use super::*;

static BENCH_CASE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) fn bench_case_id() -> u64 {
    BENCH_CASE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
}

pub(super) fn bench_paths(name: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "prodex-bench-{name}-{}-{}",
        std::process::id(),
        bench_case_id()
    ));
    AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared"),
        legacy_shared_codex_root: root.join("legacy-shared"),
        root,
    }
}

pub(super) fn bench_profile_entry(paths: &AppPaths, name: &str) -> ProfileEntry {
    ProfileEntry {
        codex_home: paths.managed_profiles_root.join(name),
        managed: true,
        email: None,
        provider: ProfileProvider::Openai,
    }
}

pub(super) fn bench_usage(
    now: i64,
    primary_used_percent: i64,
    weekly_used_percent: i64,
) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: Some("bench".to_string()),
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(primary_used_percent),
                reset_at: Some(now + 5 * 60 * 60),
                limit_window_seconds: Some(5 * 60 * 60),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(weekly_used_percent),
                reset_at: Some(now + 7 * 24 * 60 * 60),
                limit_window_seconds: Some(7 * 24 * 60 * 60),
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    }
}

pub(super) fn bench_ready_usage(now: i64) -> UsageResponse {
    bench_usage(now, 10, 20)
}

pub(super) fn bench_quota_compatible_probe_entry(now: i64) -> RuntimeProfileProbeCacheEntry {
    RuntimeProfileProbeCacheEntry {
        checked_at: now,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Err("bench".to_string()),
    }
}

pub(super) fn bench_probe_entry(
    now: i64,
    primary_used_percent: i64,
    weekly_used_percent: i64,
) -> RuntimeProfileProbeCacheEntry {
    RuntimeProfileProbeCacheEntry {
        checked_at: now,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(bench_usage(now, primary_used_percent, weekly_used_percent)),
    }
}

pub(super) fn bench_ready_probe_entry(now: i64) -> RuntimeProfileProbeCacheEntry {
    RuntimeProfileProbeCacheEntry {
        checked_at: now,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(bench_ready_usage(now)),
    }
}

pub(super) fn bench_verified_continuation_status(
    now: i64,
    route_kind: RuntimeRouteKind,
) -> RuntimeContinuationBindingStatus {
    RuntimeContinuationBindingStatus {
        state: RuntimeContinuationBindingLifecycle::Verified,
        confidence: RUNTIME_CONTINUATION_CONFIDENCE_MAX,
        last_touched_at: Some(now),
        last_verified_at: Some(now),
        last_verified_route: Some(runtime_route_kind_label(route_kind).to_string()),
        last_not_found_at: None,
        not_found_streak: 0,
        success_count: 1,
        failure_count: 0,
    }
}

pub(super) fn bench_runtime_shared(
    name: &str,
    state: RuntimeRotationState,
    active_request_limit: usize,
) -> RuntimeRotationProxyShared {
    let shared = RuntimeRotationProxyShared {
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        upstream_no_proxy: false,
        auto_redeem_enabled: false,
        compact_client: reqwest::Client::new(),
        async_client: reqwest::Client::builder()
            .build()
            .expect("benchmark async client should build"),
        async_runtime: Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("benchmark async runtime should build"),
        ),
        runtime: Arc::new(Mutex::new(state)),
        log_path: std::env::temp_dir().join(format!(
            "prodex-bench-{name}-{}-{}.log",
            std::process::id(),
            bench_case_id()
        )),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: RuntimeProxyLaneAdmission::new(runtime_proxy_lane_limits(
            active_request_limit,
            1,
            1,
        )),
    };
    register_runtime_proxy_persistence_mode(&shared.log_path, false);
    shared
}
