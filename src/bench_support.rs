use crate::*;

static BENCH_CASE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn bench_case_id() -> u64 {
    BENCH_CASE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
}

fn bench_paths(name: &str) -> AppPaths {
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

fn bench_profile_entry(paths: &AppPaths, name: &str) -> ProfileEntry {
    ProfileEntry {
        codex_home: paths.managed_profiles_root.join(name),
        managed: true,
        email: None,
        provider: ProfileProvider::Openai,
    }
}

fn bench_usage(now: i64, primary_used_percent: i64, weekly_used_percent: i64) -> UsageResponse {
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
        additional_rate_limits: Vec::new(),
    }
}

fn bench_ready_usage(now: i64) -> UsageResponse {
    bench_usage(now, 10, 20)
}

fn bench_quota_compatible_probe_entry(now: i64) -> RuntimeProfileProbeCacheEntry {
    RuntimeProfileProbeCacheEntry {
        checked_at: now,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Err("bench".to_string()),
    }
}

fn bench_probe_entry(
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

fn bench_ready_probe_entry(now: i64) -> RuntimeProfileProbeCacheEntry {
    RuntimeProfileProbeCacheEntry {
        checked_at: now,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(bench_ready_usage(now)),
    }
}

fn bench_verified_continuation_status(
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

fn bench_runtime_shared(
    name: &str,
    state: RuntimeRotationState,
    active_request_limit: usize,
) -> RuntimeRotationProxyShared {
    RuntimeRotationProxyShared {
        upstream_no_proxy: false,
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
    }
}

const DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_SAMPLES: usize = 7;
const DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_WARMUP_ITERATIONS: usize = 64;
const DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_MIN_MEASURE_ITERATIONS: usize = 32;
const DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_MAX_MEASURE_ITERATIONS: usize = 4_096;
const DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_TARGET_SAMPLE_TIME_MS: u64 = 15;
const BENCH_THRESHOLD_SCALE_DIVISOR: u64 = 100;

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProxyHotPathBenchCheckConfig {
    pub samples: usize,
    pub warmup_iterations: usize,
    pub min_measure_iterations: usize,
    pub max_measure_iterations: usize,
    pub target_sample_time: Duration,
    pub threshold_scale_percent: u64,
    pub threshold_scale_overrides: BTreeMap<String, u64>,
}

impl Default for RuntimeProxyHotPathBenchCheckConfig {
    fn default() -> Self {
        Self {
            samples: DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_SAMPLES,
            warmup_iterations: DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_WARMUP_ITERATIONS,
            min_measure_iterations: DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_MIN_MEASURE_ITERATIONS,
            max_measure_iterations: DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_MAX_MEASURE_ITERATIONS,
            target_sample_time: Duration::from_millis(
                DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_TARGET_SAMPLE_TIME_MS,
            ),
            threshold_scale_percent: BENCH_THRESHOLD_SCALE_DIVISOR,
            threshold_scale_overrides: BTreeMap::new(),
        }
    }
}

impl RuntimeProxyHotPathBenchCheckConfig {
    pub fn with_threshold_scale_percent(mut self, threshold_scale_percent: u64) -> Self {
        self.threshold_scale_percent = threshold_scale_percent.max(1);
        self
    }

    pub fn with_threshold_scale_overrides<I, K>(mut self, threshold_scale_overrides: I) -> Self
    where
        I: IntoIterator<Item = (K, u64)>,
        K: Into<String>,
    {
        self.threshold_scale_overrides = threshold_scale_overrides
            .into_iter()
            .map(|(case_name, scale_percent)| (case_name.into(), scale_percent.max(1)))
            .collect();
        self
    }

    fn threshold_scale_percent_for(&self, case_name: &str) -> u64 {
        self.threshold_scale_overrides
            .get(case_name)
            .copied()
            .unwrap_or(self.threshold_scale_percent)
            .max(1)
    }

    fn normalized(self) -> Self {
        let min_measure_iterations = self.min_measure_iterations.max(1);
        let max_measure_iterations = self.max_measure_iterations.max(min_measure_iterations);
        let target_sample_time = if self.target_sample_time.is_zero() {
            Duration::from_millis(DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_TARGET_SAMPLE_TIME_MS)
        } else {
            self.target_sample_time
        };

        Self {
            samples: self.samples.max(1),
            warmup_iterations: self.warmup_iterations.max(1),
            min_measure_iterations,
            max_measure_iterations,
            target_sample_time,
            threshold_scale_percent: self.threshold_scale_percent.max(1),
            threshold_scale_overrides: self
                .threshold_scale_overrides
                .into_iter()
                .map(|(case_name, scale_percent)| (case_name, scale_percent.max(1)))
                .collect(),
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeProxyHotPathBenchCheckResult {
    pub name: &'static str,
    pub samples: usize,
    pub warmup_iterations: usize,
    pub measure_iterations: usize,
    pub base_threshold_ns_per_iteration: u64,
    pub threshold_scale_percent: u64,
    pub median_ns_per_iteration: u64,
    pub p90_ns_per_iteration: u64,
    pub threshold_ns_per_iteration: u64,
}

impl RuntimeProxyHotPathBenchCheckResult {
    pub fn passed(&self) -> bool {
        self.median_ns_per_iteration <= self.threshold_ns_per_iteration
    }

    pub fn required_threshold_scale_percent(&self) -> u64 {
        if self.base_threshold_ns_per_iteration == 0 {
            return u64::MAX;
        }
        let required_percent = self
            .median_ns_per_iteration
            .saturating_mul(BENCH_THRESHOLD_SCALE_DIVISOR)
            .saturating_add(self.base_threshold_ns_per_iteration - 1)
            / self.base_threshold_ns_per_iteration;
        required_percent.max(1)
    }

    pub fn scale_headroom_percent(&self) -> i128 {
        i128::from(self.threshold_scale_percent)
            - i128::from(self.required_threshold_scale_percent())
    }

    pub fn threshold_headroom_ns_per_iteration(&self) -> i128 {
        i128::from(self.threshold_ns_per_iteration) - i128::from(self.median_ns_per_iteration)
    }
}

#[derive(Clone, Copy)]
struct RuntimeProxyHotPathBenchThreshold {
    name: &'static str,
    max_median_ns_per_iteration: u64,
}

const RUNTIME_PROXY_QUOTA_FALLBACK_BENCH_THRESHOLD: RuntimeProxyHotPathBenchThreshold =
    RuntimeProxyHotPathBenchThreshold {
        name: "runtime_route_eligible_quota_fallback_scan",
        max_median_ns_per_iteration: 30_000,
    };
const RUNTIME_PROXY_PREVIOUS_RESPONSE_BENCH_THRESHOLD: RuntimeProxyHotPathBenchThreshold =
    RuntimeProxyHotPathBenchThreshold {
        name: "runtime_previous_response_candidate_selection",
        max_median_ns_per_iteration: 110_000,
    };
const RUNTIME_PROXY_MIXED_POOL_BENCH_THRESHOLD: RuntimeProxyHotPathBenchThreshold =
    RuntimeProxyHotPathBenchThreshold {
        name: "runtime_mixed_pool_response_selection",
        max_median_ns_per_iteration: 2_400_000,
    };
const RUNTIME_PROXY_COMPACT_SESSION_SELECTION_BENCH_THRESHOLD: RuntimeProxyHotPathBenchThreshold =
    RuntimeProxyHotPathBenchThreshold {
        name: "runtime_compact_session_affinity_selection",
        max_median_ns_per_iteration: 160_000,
    };
const RUNTIME_PROXY_WEBSOCKET_STALE_REUSE_BENCH_THRESHOLD: RuntimeProxyHotPathBenchThreshold =
    RuntimeProxyHotPathBenchThreshold {
        name: "runtime_websocket_stale_reuse_affinity",
        max_median_ns_per_iteration: 220_000,
    };
const RUNTIME_PROXY_SSE_INSPECT_BENCH_THRESHOLD: RuntimeProxyHotPathBenchThreshold =
    RuntimeProxyHotPathBenchThreshold {
        name: "runtime_sse_lookahead_inspection",
        max_median_ns_per_iteration: 130_000,
    };
const RUNTIME_PROXY_LINEAGE_CLEANUP_BENCH_THRESHOLD: RuntimeProxyHotPathBenchThreshold =
    RuntimeProxyHotPathBenchThreshold {
        name: "runtime_dead_lineage_cleanup",
        max_median_ns_per_iteration: 190_000,
    };

fn scaled_runtime_proxy_hot_path_threshold_ns(
    threshold: RuntimeProxyHotPathBenchThreshold,
    threshold_scale_percent: u64,
) -> u64 {
    threshold
        .max_median_ns_per_iteration
        .saturating_mul(threshold_scale_percent.max(1))
        / BENCH_THRESHOLD_SCALE_DIVISOR
}

fn measure_runtime_proxy_hot_path_batch<T, F>(iterations: usize, mut step: F) -> u128
where
    F: FnMut() -> T,
{
    let started_at = Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(step());
    }
    started_at.elapsed().as_nanos()
}

struct RuntimeProxyHotPathSummaryInput {
    name: &'static str,
    samples: usize,
    warmup_iterations: usize,
    measure_iterations: usize,
    base_threshold_ns_per_iteration: u64,
    threshold_scale_percent: u64,
    threshold_ns_per_iteration: u64,
    ns_per_iteration_samples: Vec<u64>,
}

fn summarize_runtime_proxy_hot_path_samples(
    input: RuntimeProxyHotPathSummaryInput,
) -> RuntimeProxyHotPathBenchCheckResult {
    let RuntimeProxyHotPathSummaryInput {
        name,
        samples,
        warmup_iterations,
        measure_iterations,
        base_threshold_ns_per_iteration,
        threshold_scale_percent,
        threshold_ns_per_iteration,
        mut ns_per_iteration_samples,
    } = input;
    ns_per_iteration_samples.sort_unstable();
    let median_ns_per_iteration = ns_per_iteration_samples[ns_per_iteration_samples.len() / 2];
    let p90_index = ((ns_per_iteration_samples.len() - 1) * 9) / 10;
    let p90_ns_per_iteration = ns_per_iteration_samples[p90_index];

    RuntimeProxyHotPathBenchCheckResult {
        name,
        samples,
        warmup_iterations,
        measure_iterations,
        base_threshold_ns_per_iteration,
        threshold_scale_percent,
        median_ns_per_iteration,
        p90_ns_per_iteration,
        threshold_ns_per_iteration,
    }
}

fn run_runtime_proxy_hot_path_case<T, F>(
    config: RuntimeProxyHotPathBenchCheckConfig,
    threshold: RuntimeProxyHotPathBenchThreshold,
    mut step: F,
) -> RuntimeProxyHotPathBenchCheckResult
where
    F: FnMut() -> T,
{
    let threshold_scale_percent = config.threshold_scale_percent_for(threshold.name);
    let threshold_ns_per_iteration =
        scaled_runtime_proxy_hot_path_threshold_ns(threshold, threshold_scale_percent);
    for _ in 0..config.warmup_iterations {
        std::hint::black_box(step());
    }

    let pilot_iterations = config.min_measure_iterations.min(64);
    let pilot_elapsed_ns = measure_runtime_proxy_hot_path_batch(pilot_iterations, &mut step);
    let pilot_ns_per_iteration =
        (pilot_elapsed_ns / u128::from(pilot_iterations.max(1) as u64)).max(1);
    let desired_iterations = (config.target_sample_time.as_nanos() / pilot_ns_per_iteration)
        .try_into()
        .unwrap_or(usize::MAX);
    let measure_iterations =
        desired_iterations.clamp(config.min_measure_iterations, config.max_measure_iterations);

    let mut ns_per_iteration_samples = Vec::with_capacity(config.samples);
    for _ in 0..config.samples {
        let elapsed_ns = measure_runtime_proxy_hot_path_batch(measure_iterations, &mut step);
        let ns_per_iteration = (elapsed_ns / u128::from(measure_iterations as u64))
            .try_into()
            .unwrap_or(u64::MAX);
        ns_per_iteration_samples.push(ns_per_iteration);
    }

    summarize_runtime_proxy_hot_path_samples(RuntimeProxyHotPathSummaryInput {
        name: threshold.name,
        samples: config.samples,
        warmup_iterations: config.warmup_iterations,
        measure_iterations,
        base_threshold_ns_per_iteration: threshold.max_median_ns_per_iteration,
        threshold_scale_percent,
        threshold_ns_per_iteration,
        ns_per_iteration_samples,
    })
}

#[doc(hidden)]
pub fn run_runtime_proxy_hot_path_bench_check(
    config: RuntimeProxyHotPathBenchCheckConfig,
) -> Vec<RuntimeProxyHotPathBenchCheckResult> {
    let config = config.normalized();

    let quota_fallback = RuntimeProxyQuotaFallbackBenchCase::new(64);
    let previous_response = RuntimeProxyPreviousResponseBenchCase::new(64);
    let mixed_pool_selection = RuntimeProxyMixedPoolSelectionBenchCase::new(96);
    let compact_session_selection = RuntimeProxyCompactSessionSelectionBenchCase::new(64);
    let websocket_stale_reuse = RuntimeProxyWebsocketStaleReuseBenchCase::new(64);
    let sse_inspect = RuntimeProxySseInspectBenchCase::new(128);
    let lineage_cleanup = RuntimeProxyLineageCleanupBenchCase::new(128);

    vec![
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_QUOTA_FALLBACK_BENCH_THRESHOLD,
            || quota_fallback.has_route_eligible_quota_fallback(),
        ),
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_PREVIOUS_RESPONSE_BENCH_THRESHOLD,
            || previous_response.next_previous_response_candidate(),
        ),
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_MIXED_POOL_BENCH_THRESHOLD,
            || mixed_pool_selection.select_fresh_response_candidate(),
        ),
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_COMPACT_SESSION_SELECTION_BENCH_THRESHOLD,
            || compact_session_selection.select_compact_session_candidate(),
        ),
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_WEBSOCKET_STALE_REUSE_BENCH_THRESHOLD,
            || websocket_stale_reuse.evaluate_stale_reuse_affinity(),
        ),
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_SSE_INSPECT_BENCH_THRESHOLD,
            || sse_inspect.inspect(),
        ),
        run_runtime_proxy_hot_path_case(
            config,
            RUNTIME_PROXY_LINEAGE_CLEANUP_BENCH_THRESHOLD,
            || lineage_cleanup.clear_dead_response_bindings(),
        ),
    ]
}

#[doc(hidden)]
pub struct RuntimeProxyQuotaFallbackBenchCase {
    shared: RuntimeRotationProxyShared,
    excluded_profiles: BTreeSet<String>,
    profile_name: String,
}

impl RuntimeProxyQuotaFallbackBenchCase {
    pub fn new(profile_count: usize) -> Self {
        let profile_count = profile_count.max(2);
        let paths = bench_paths("quota-fallback");
        let now = Local::now().timestamp();
        let mut profiles = BTreeMap::new();
        let mut probe_cache = BTreeMap::new();
        let mut profile_inflight = BTreeMap::new();

        for index in 0..profile_count {
            let name = format!("profile-{index:03}");
            profiles.insert(name.clone(), bench_profile_entry(&paths, &name));
            probe_cache.insert(name.clone(), bench_quota_compatible_probe_entry(now));
            if index > 0 && index + 1 < profile_count {
                profile_inflight.insert(name, usize::MAX / 4);
            }
        }

        let profile_name = "profile-000".to_string();
        let state = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(profile_name.clone()),
                profiles,
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: profile_name.clone(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: probe_cache,
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight,
            profile_health: BTreeMap::new(),
        };

        Self {
            shared: bench_runtime_shared("quota-fallback", state, 32),
            excluded_profiles: BTreeSet::new(),
            profile_name,
        }
    }

    pub fn has_route_eligible_quota_fallback(&self) -> bool {
        runtime_has_route_eligible_quota_fallback(
            &self.shared,
            &self.profile_name,
            &self.excluded_profiles,
            RuntimeRouteKind::Responses,
        )
        .expect("benchmark selection should succeed")
    }
}

#[doc(hidden)]
pub struct RuntimeProxyPreviousResponseBenchCase {
    shared: RuntimeRotationProxyShared,
    excluded_profiles: BTreeSet<String>,
    previous_response_id: String,
}

impl RuntimeProxyPreviousResponseBenchCase {
    pub fn new(profile_count: usize) -> Self {
        let profile_count = profile_count.max(16);
        let paths = bench_paths("previous-response-selection");
        let now = Local::now().timestamp();
        let mut profiles = BTreeMap::new();
        let mut probe_cache = BTreeMap::new();
        let mut last_run_selected_at = BTreeMap::new();
        let mut profile_health = BTreeMap::new();
        let previous_response_id = "resp-bench".to_string();

        for index in 0..profile_count {
            let name = format!("profile-{index:03}");
            profiles.insert(name.clone(), bench_profile_entry(&paths, &name));
            probe_cache.insert(
                name.clone(),
                if index % 7 == 0 {
                    bench_probe_entry(now, 96, 96)
                } else {
                    bench_ready_probe_entry(now)
                },
            );
            last_run_selected_at.insert(name.clone(), now - index as i64);
            if index < profile_count / 3 {
                profile_health.insert(
                    runtime_previous_response_negative_cache_key(
                        &previous_response_id,
                        &name,
                        RuntimeRouteKind::Responses,
                    ),
                    RuntimeProfileHealth {
                        score: 1,
                        updated_at: now,
                    },
                );
            }
        }

        let current_profile = "profile-000".to_string();
        let state = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(current_profile.clone()),
                profiles,
                last_run_selected_at,
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile,
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: probe_cache,
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health,
        };

        Self {
            shared: bench_runtime_shared("previous-response-selection", state, 32),
            excluded_profiles: BTreeSet::new(),
            previous_response_id,
        }
    }

    pub fn next_previous_response_candidate(&self) -> Option<String> {
        next_runtime_previous_response_candidate(
            &self.shared,
            &self.excluded_profiles,
            Some(&self.previous_response_id),
            RuntimeRouteKind::Responses,
        )
        .expect("benchmark previous response selection should succeed")
    }
}

#[doc(hidden)]
pub struct RuntimeProxyMixedPoolSelectionBenchCase {
    shared: RuntimeRotationProxyShared,
    excluded_profiles: BTreeSet<String>,
}

impl RuntimeProxyMixedPoolSelectionBenchCase {
    pub fn new(profile_count: usize) -> Self {
        let profile_count = profile_count.max(32);
        let paths = bench_paths("mixed-pool-selection");
        let now = Local::now().timestamp();
        let mut profiles = BTreeMap::new();
        let mut probe_cache = BTreeMap::new();
        let mut last_run_selected_at = BTreeMap::new();
        let mut retry_backoff_until = BTreeMap::new();
        let mut transport_backoff_until = BTreeMap::new();
        let mut route_circuit_open_until = BTreeMap::new();
        let mut profile_inflight = BTreeMap::new();
        let mut profile_health = BTreeMap::new();
        let mut excluded_profiles = BTreeSet::new();

        for index in 0..profile_count {
            let name = format!("profile-{index:03}");
            profiles.insert(name.clone(), bench_profile_entry(&paths, &name));
            let (primary_used, weekly_used) = match index % 12 {
                1 | 2 => (97, 97),
                3 => (88, 94),
                _ => (10 + (index % 20) as i64, 20 + (index % 15) as i64),
            };
            probe_cache.insert(
                name.clone(),
                bench_probe_entry(now, primary_used, weekly_used),
            );
            last_run_selected_at.insert(name.clone(), now - index as i64);

            if index < profile_count / 8 {
                excluded_profiles.insert(name.clone());
            } else if index < profile_count / 3 {
                match index % 5 {
                    0 => {
                        retry_backoff_until.insert(name.clone(), now + 90);
                    }
                    1 => {
                        transport_backoff_until.insert(
                            runtime_profile_transport_backoff_key(
                                &name,
                                RuntimeRouteKind::Responses,
                            ),
                            now + 45,
                        );
                    }
                    2 => {
                        route_circuit_open_until.insert(
                            runtime_profile_route_circuit_key(&name, RuntimeRouteKind::Responses),
                            now + 30,
                        );
                    }
                    3 => {
                        profile_inflight.insert(name.clone(), usize::MAX / 4);
                    }
                    _ => {
                        profile_health.insert(
                            runtime_profile_route_health_key(&name, RuntimeRouteKind::Responses),
                            RuntimeProfileHealth {
                                score: 3,
                                updated_at: now,
                            },
                        );
                    }
                }
            } else if index % 17 == 0 {
                profile_inflight.insert(name.clone(), 2);
            }
        }

        let current_profile = "profile-000".to_string();
        let state = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(current_profile.clone()),
                profiles,
                last_run_selected_at,
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile,
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: probe_cache,
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: retry_backoff_until,
            profile_transport_backoff_until: transport_backoff_until,
            profile_route_circuit_open_until: route_circuit_open_until,
            profile_inflight,
            profile_health,
        };

        Self {
            shared: bench_runtime_shared("mixed-pool-selection", state, 64),
            excluded_profiles,
        }
    }

    pub fn select_fresh_response_candidate(&self) -> Option<String> {
        select_runtime_response_candidate_for_route(
            &self.shared,
            &self.excluded_profiles,
            None,
            None,
            None,
            None,
            false,
            None,
            RuntimeRouteKind::Responses,
        )
        .expect("benchmark mixed-pool selection should succeed")
    }
}

#[doc(hidden)]
pub struct RuntimeProxyCompactSessionSelectionBenchCase {
    shared: RuntimeRotationProxyShared,
    excluded_profiles: BTreeSet<String>,
    session_id: String,
}

impl RuntimeProxyCompactSessionSelectionBenchCase {
    pub fn new(profile_count: usize) -> Self {
        let profile_count = profile_count.max(16);
        let paths = bench_paths("compact-session-selection");
        let now = Local::now().timestamp();
        let mut profiles = BTreeMap::new();
        let mut probe_cache = BTreeMap::new();
        let mut last_run_selected_at = BTreeMap::new();
        let mut profile_inflight = BTreeMap::new();
        let session_id = "session-bench".to_string();
        let session_profile = format!("profile-{:03}", profile_count / 2);

        for index in 0..profile_count {
            let name = format!("profile-{index:03}");
            profiles.insert(name.clone(), bench_profile_entry(&paths, &name));
            probe_cache.insert(name.clone(), bench_ready_probe_entry(now));
            last_run_selected_at.insert(name.clone(), now - index as i64);
            if name == session_profile {
                profile_inflight.insert(name, usize::MAX / 4);
            }
        }

        let binding = ResponseProfileBinding {
            profile_name: session_profile,
            bound_at: now,
        };
        let mut session_id_bindings = BTreeMap::new();
        session_id_bindings.insert(session_id.clone(), binding.clone());
        let mut session_profile_bindings = BTreeMap::new();
        session_profile_bindings.insert(session_id.clone(), binding);
        let mut continuation_statuses = RuntimeContinuationStatuses::default();
        continuation_statuses.session_id.insert(
            session_id.clone(),
            bench_verified_continuation_status(now, RuntimeRouteKind::Compact),
        );

        let current_profile = "profile-000".to_string();
        let state = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(current_profile.clone()),
                profiles,
                last_run_selected_at,
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings,
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile,
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings,
            continuation_statuses,
            profile_probe_cache: probe_cache,
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight,
            profile_health: BTreeMap::new(),
        };

        Self {
            shared: bench_runtime_shared("compact-session-selection", state, 32),
            excluded_profiles: BTreeSet::new(),
            session_id,
        }
    }

    pub fn select_compact_session_candidate(&self) -> Option<String> {
        let session_profile = runtime_session_bound_profile(&self.shared, &self.session_id)
            .expect("benchmark compact session lookup should succeed");
        select_runtime_response_candidate_for_route(
            &self.shared,
            &self.excluded_profiles,
            None,
            None,
            None,
            session_profile.as_deref(),
            false,
            None,
            RuntimeRouteKind::Compact,
        )
        .expect("benchmark compact session selection should succeed")
    }
}

#[doc(hidden)]
pub struct RuntimeProxyWebsocketStaleReuseBenchCase {
    shared: RuntimeRotationProxyShared,
    excluded_profiles: BTreeSet<String>,
    previous_response_id: String,
    stale_elapsed: Duration,
}

impl RuntimeProxyWebsocketStaleReuseBenchCase {
    pub fn new(profile_count: usize) -> Self {
        let profile_count = profile_count.max(16);
        let paths = bench_paths("websocket-stale-reuse");
        let now = Local::now().timestamp();
        let mut profiles = BTreeMap::new();
        let mut probe_cache = BTreeMap::new();
        let mut last_run_selected_at = BTreeMap::new();
        let previous_response_id = "resp-websocket-bench".to_string();
        let owner_profile = format!("profile-{:03}", profile_count / 3);

        for index in 0..profile_count {
            let name = format!("profile-{index:03}");
            profiles.insert(name.clone(), bench_profile_entry(&paths, &name));
            probe_cache.insert(name.clone(), bench_ready_probe_entry(now));
            last_run_selected_at.insert(name, now - index as i64);
        }

        let mut response_profile_bindings = BTreeMap::new();
        response_profile_bindings.insert(
            previous_response_id.clone(),
            ResponseProfileBinding {
                profile_name: owner_profile,
                bound_at: now,
            },
        );
        let mut continuation_statuses = RuntimeContinuationStatuses::default();
        continuation_statuses.response.insert(
            previous_response_id.clone(),
            bench_verified_continuation_status(now, RuntimeRouteKind::Websocket),
        );

        let current_profile = "profile-000".to_string();
        let state = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(current_profile.clone()),
                profiles,
                last_run_selected_at,
                response_profile_bindings,
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile,
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses,
            profile_probe_cache: probe_cache,
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        };

        Self {
            shared: bench_runtime_shared("websocket-stale-reuse", state, 32),
            excluded_profiles: BTreeSet::new(),
            previous_response_id,
            stale_elapsed: Duration::from_millis(
                RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS.saturating_add(1),
            ),
        }
    }

    pub fn evaluate_stale_reuse_affinity(&self) -> usize {
        let bound_profile = runtime_response_bound_profile(
            &self.shared,
            &self.previous_response_id,
            RuntimeRouteKind::Websocket,
        )
        .expect("benchmark websocket response binding lookup should succeed");
        let trusted = runtime_previous_response_affinity_is_trusted(
            &self.shared,
            Some(&self.previous_response_id),
            bound_profile.as_deref(),
        )
        .expect("benchmark websocket affinity trust lookup should succeed");
        let nonreplayable = runtime_websocket_previous_response_reuse_is_nonreplayable(
            Some(&self.previous_response_id),
            false,
            None,
        );
        let stale = runtime_websocket_previous_response_reuse_is_stale(
            nonreplayable,
            Some(self.stale_elapsed),
        );
        let fallback_allowed =
            runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed(
                RuntimeWebsocketReuseWatchdogPreviousResponseFallback {
                    profile_name: bound_profile.as_deref().unwrap_or(""),
                    previous_response_id: Some(&self.previous_response_id),
                    previous_response_fresh_fallback_used: false,
                    bound_profile: bound_profile.as_deref(),
                    pinned_profile: bound_profile.as_deref(),
                    request_requires_previous_response_affinity: false,
                    trusted_previous_response_affinity: trusted,
                    request_turn_state: None,
                    fresh_fallback_shape: Some(
                        RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
                    ),
                },
            );
        let candidate = select_runtime_response_candidate_for_route(
            &self.shared,
            &self.excluded_profiles,
            None,
            bound_profile.as_deref(),
            None,
            None,
            true,
            Some(&self.previous_response_id),
            RuntimeRouteKind::Websocket,
        )
        .expect("benchmark websocket stale reuse selection should succeed");

        usize::from(trusted)
            + usize::from(nonreplayable)
            + usize::from(stale)
            + usize::from(!fallback_allowed)
            + usize::from(candidate.as_deref() == bound_profile.as_deref())
    }
}

#[doc(hidden)]
pub struct RuntimeProxySseInspectBenchCase {
    buffer: Vec<u8>,
}

impl RuntimeProxySseInspectBenchCase {
    pub fn new(event_count: usize) -> Self {
        let event_count = event_count.max(1);
        let mut buffer = Vec::new();
        for index in 0..event_count {
            if index % 8 == 0 {
                buffer.extend_from_slice(b": keep-alive\r\n");
            }
            let event_type = match index % 6 {
                0 => "response.created",
                1 => "response.in_progress",
                2 => "response.output_item.added",
                3 => "response.content_part.added",
                4 => "response.output_text.delta",
                _ => "response.reasoning_summary_text.delta",
            };
            buffer.extend_from_slice(
                format!(
                    "event: {event_type}\r\ndata: {{\"type\":\"{event_type}\",\"response_id\":\"resp-{index:03}\",\"delta\":\"bench-token-{index:03}\"}}\r\n\r\n"
                )
                .as_bytes(),
            );
        }
        buffer.extend_from_slice(
            b"event: response.completed\r\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-tail\",\"turn_state\":\"turn-tail\"}}\r\n\r\n",
        );
        Self { buffer }
    }

    pub fn inspect(&self) -> usize {
        match inspect_runtime_sse_buffer(&self.buffer).expect("benchmark SSE parse should succeed")
        {
            RuntimeSseInspectionProgress::Hold {
                response_ids,
                turn_state,
            }
            | RuntimeSseInspectionProgress::Commit {
                response_ids,
                turn_state,
            } => response_ids.len() + usize::from(turn_state.is_some()),
            RuntimeSseInspectionProgress::QuotaBlocked
            | RuntimeSseInspectionProgress::PreviousResponseNotFound => 0,
        }
    }
}

#[doc(hidden)]
pub struct RuntimeProxyLineageCleanupBenchCase {
    shared: RuntimeRotationProxyShared,
    template: RuntimeRotationState,
    profile_name: String,
    response_ids: Vec<String>,
}

impl RuntimeProxyLineageCleanupBenchCase {
    pub fn new(turn_state_count: usize) -> Self {
        let turn_state_count = turn_state_count.max(2);
        let paths = bench_paths("lineage-cleanup");
        let now = Local::now().timestamp();
        let profile_name = "main".to_string();
        let target_response_id = "resp-target".to_string();
        let mut response_profile_bindings = BTreeMap::new();
        let mut turn_state_bindings = BTreeMap::new();

        response_profile_bindings.insert(
            target_response_id.clone(),
            ResponseProfileBinding {
                profile_name: profile_name.clone(),
                bound_at: now,
            },
        );

        for index in 0..turn_state_count {
            let turn_state = format!("turn-{index:03}");
            turn_state_bindings.insert(
                turn_state.clone(),
                ResponseProfileBinding {
                    profile_name: profile_name.clone(),
                    bound_at: now,
                },
            );
            response_profile_bindings.insert(
                runtime_response_turn_state_lineage_key(&target_response_id, &turn_state),
                ResponseProfileBinding {
                    profile_name: profile_name.clone(),
                    bound_at: now,
                },
            );
            if index % 2 == 0 {
                let survivor_response_id = format!("resp-survivor-{index:03}");
                response_profile_bindings.insert(
                    survivor_response_id.clone(),
                    ResponseProfileBinding {
                        profile_name: profile_name.clone(),
                        bound_at: now,
                    },
                );
                response_profile_bindings.insert(
                    runtime_response_turn_state_lineage_key(&survivor_response_id, &turn_state),
                    ResponseProfileBinding {
                        profile_name: profile_name.clone(),
                        bound_at: now,
                    },
                );
            }
        }

        let template = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(profile_name.clone()),
                profiles: BTreeMap::from([(
                    profile_name.clone(),
                    ProfileEntry {
                        codex_home: PathBuf::from("/tmp/prodex-bench/main"),
                        managed: true,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings,
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: profile_name.clone(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings,
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        };

        Self {
            shared: bench_runtime_shared("lineage-cleanup", template.clone(), 8),
            template,
            profile_name,
            response_ids: vec![target_response_id],
        }
    }

    pub fn clear_dead_response_bindings(&self) -> usize {
        let mut runtime = self
            .shared
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *runtime = self.template.clone();
        drop(runtime);

        clear_runtime_dead_response_bindings(
            &self.shared,
            &self.profile_name,
            &self.response_ids,
            "bench_cleanup",
        )
        .expect("benchmark lineage cleanup should succeed");

        let runtime = self
            .shared
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.turn_state_bindings.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_proxy_hot_path_bench_check_config_normalizes_zero_values() {
        let config = RuntimeProxyHotPathBenchCheckConfig {
            samples: 0,
            warmup_iterations: 0,
            min_measure_iterations: 0,
            max_measure_iterations: 0,
            target_sample_time: Duration::ZERO,
            threshold_scale_percent: 0,
            threshold_scale_overrides: BTreeMap::from([("runtime_case".to_string(), 0)]),
        }
        .normalized();

        assert_eq!(config.samples, 1);
        assert_eq!(config.warmup_iterations, 1);
        assert_eq!(config.min_measure_iterations, 1);
        assert_eq!(config.max_measure_iterations, 1);
        assert_eq!(
            config.target_sample_time,
            Duration::from_millis(DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_TARGET_SAMPLE_TIME_MS)
        );
        assert_eq!(config.threshold_scale_percent, 1);
        assert_eq!(config.threshold_scale_percent_for("runtime_case"), 1);
    }

    #[test]
    fn runtime_proxy_hot_path_bench_config_uses_case_override_thresholds() {
        let config = RuntimeProxyHotPathBenchCheckConfig::default()
            .with_threshold_scale_percent(150)
            .with_threshold_scale_overrides([("runtime_case", 180), ("runtime_other", 90)]);

        assert_eq!(config.threshold_scale_percent_for("runtime_case"), 180);
        assert_eq!(config.threshold_scale_percent_for("runtime_other"), 90);
        assert_eq!(config.threshold_scale_percent_for("runtime_missing"), 150);
    }

    #[test]
    fn runtime_proxy_hot_path_bench_summary_uses_sorted_percentiles() {
        let result = summarize_runtime_proxy_hot_path_samples(RuntimeProxyHotPathSummaryInput {
            name: "runtime_case",
            samples: 5,
            warmup_iterations: 32,
            measure_iterations: 128,
            base_threshold_ns_per_iteration: 10,
            threshold_scale_percent: 70,
            threshold_ns_per_iteration: 7,
            ns_per_iteration_samples: vec![9, 3, 5, 1, 7],
        });

        assert_eq!(result.name, "runtime_case");
        assert_eq!(result.samples, 5);
        assert_eq!(result.warmup_iterations, 32);
        assert_eq!(result.measure_iterations, 128);
        assert_eq!(result.base_threshold_ns_per_iteration, 10);
        assert_eq!(result.threshold_scale_percent, 70);
        assert_eq!(result.median_ns_per_iteration, 5);
        assert_eq!(result.p90_ns_per_iteration, 7);
        assert_eq!(result.required_threshold_scale_percent(), 50);
        assert_eq!(result.scale_headroom_percent(), 20);
        assert_eq!(result.threshold_headroom_ns_per_iteration(), 2);
        assert!(result.passed());
    }
}
