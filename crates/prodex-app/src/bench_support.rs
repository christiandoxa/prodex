use crate::*;
pub use prodex_bench_support::{
    RUNTIME_PROXY_COMPACT_SESSION_SELECTION_BENCH_CASE, RUNTIME_PROXY_HOT_PATH_BENCH_CASE_SPECS,
    RUNTIME_PROXY_LINEAGE_CLEANUP_BENCH_CASE, RUNTIME_PROXY_MIXED_POOL_BENCH_CASE,
    RUNTIME_PROXY_PREVIOUS_RESPONSE_BENCH_CASE, RUNTIME_PROXY_QUOTA_FALLBACK_BENCH_CASE,
    RUNTIME_PROXY_RUNTIME_MEM_SUPER_SLIM_BENCH_CASE,
    RUNTIME_PROXY_SMART_CONTEXT_REWRITE_BENCH_CASE, RUNTIME_PROXY_SSE_INSPECT_BENCH_CASE,
    RUNTIME_PROXY_WEBSOCKET_STALE_REUSE_BENCH_CASE, RuntimeProxyHotPathBenchCaseSpec,
    RuntimeProxyHotPathBenchCaseSuite, RuntimeProxyHotPathBenchCheckConfig,
    RuntimeProxyHotPathBenchCheckResult, RuntimeProxyHotPathBenchScenarioSizes,
    RuntimeProxyHotPathBenchThreshold, run_runtime_proxy_hot_path_case,
    run_runtime_proxy_hot_path_case_suite,
};

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

#[doc(hidden)]
pub fn run_runtime_proxy_hot_path_bench_check(
    config: RuntimeProxyHotPathBenchCheckConfig,
) -> Vec<RuntimeProxyHotPathBenchCheckResult> {
    let sizes = RuntimeProxyHotPathBenchScenarioSizes::default();
    let quota_fallback = RuntimeProxyQuotaFallbackBenchCase::new(sizes.quota_fallback);
    let previous_response = RuntimeProxyPreviousResponseBenchCase::new(sizes.previous_response);
    let mixed_pool_selection =
        RuntimeProxyMixedPoolSelectionBenchCase::new(sizes.mixed_pool_selection);
    let compact_session_selection =
        RuntimeProxyCompactSessionSelectionBenchCase::new(sizes.compact_session_selection);
    let websocket_stale_reuse =
        RuntimeProxyWebsocketStaleReuseBenchCase::new(sizes.websocket_stale_reuse);
    let sse_inspect = RuntimeProxySseInspectBenchCase::new(sizes.sse_inspect);
    let lineage_cleanup = RuntimeProxyLineageCleanupBenchCase::new(sizes.lineage_cleanup);
    let smart_context_rewrite =
        RuntimeProxySmartContextRewriteBenchCase::new(sizes.smart_context_rewrite);
    let runtime_mem_super_slim =
        RuntimeProxyRuntimeMemSuperSlimBenchCase::new(sizes.runtime_mem_super_slim);

    run_runtime_proxy_hot_path_case_suite(
        config,
        RuntimeProxyHotPathBenchCaseSuite {
            quota_fallback: || quota_fallback.has_route_eligible_quota_fallback(),
            previous_response: || previous_response.next_previous_response_candidate(),
            mixed_pool_selection: || mixed_pool_selection.select_fresh_response_candidate(),
            compact_session_selection: || {
                compact_session_selection.select_compact_session_candidate()
            },
            websocket_stale_reuse: || websocket_stale_reuse.evaluate_stale_reuse_affinity(),
            sse_inspect: || sse_inspect.inspect(),
            lineage_cleanup: || lineage_cleanup.clear_dead_response_bindings(),
            smart_context_rewrite: || smart_context_rewrite.rewrite_large_tool_output(),
            runtime_mem_super_slim: || runtime_mem_super_slim.shadow_token_heavy_events(),
        },
    )
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
            RuntimeResponseCandidateSelection::fresh(
                &self.excluded_profiles,
                RuntimeRouteKind::Responses,
            ),
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
            RuntimeResponseCandidateSelection {
                session_profile: session_profile.as_deref(),
                ..RuntimeResponseCandidateSelection::fresh(
                    &self.excluded_profiles,
                    RuntimeRouteKind::Compact,
                )
            },
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
            RuntimeResponseCandidateSelection {
                pinned_profile: bound_profile.as_deref(),
                discover_previous_response_owner: true,
                previous_response_id: Some(&self.previous_response_id),
                ..RuntimeResponseCandidateSelection::fresh(
                    &self.excluded_profiles,
                    RuntimeRouteKind::Websocket,
                )
            },
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
        match inspect_runtime_sse_buffer(&self.buffer) {
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

#[doc(hidden)]
pub struct RuntimeProxySmartContextRewriteBenchCase {
    shared: RuntimeRotationProxyShared,
    request: RuntimeProxyRequest,
}

impl RuntimeProxySmartContextRewriteBenchCase {
    pub fn new(tool_line_count: usize) -> Self {
        let tool_line_count = tool_line_count.max(32);
        let paths = bench_paths("smart-context-rewrite");
        let profile_name = "main".to_string();
        let state = RuntimeRotationState {
            paths: paths.clone(),
            state: AppState {
                active_profile: Some(profile_name.clone()),
                profiles: BTreeMap::from([(
                    profile_name.clone(),
                    bench_profile_entry(&paths, &profile_name),
                )]),
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
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        };
        let shared = bench_runtime_shared("smart-context-rewrite", state, 8);
        register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(18_000), None);

        let output = (0..tool_line_count)
            .map(|index| {
                format!(
                    "line {index:04}: /repo/prodex/crates/prodex-app/src/runtime_proxy/smart_context.rs token-heavy-tool-output repeated-context payload={}",
                    "abcdef0123456789".repeat(8)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gpt-5.1-codex",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": "Summarize the failing runtime proxy tool output and keep artifact references."
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_bench_large_tool_output",
                    "output": output
                }
            ]
        }))
        .expect("benchmark smart-context request body should serialize");
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/responses".to_string(),
            headers: Vec::new(),
            body,
        };

        Self { shared, request }
    }

    pub fn rewrite_large_tool_output(&self) -> usize {
        prepare_runtime_smart_context_http_body_for_profile(
            bench_case_id(),
            &self.request,
            &self.shared,
            RuntimeRouteKind::Responses,
            Some("main"),
        )
        .len()
    }
}

#[doc(hidden)]
pub struct RuntimeProxyRuntimeMemSuperSlimBenchCase {
    events: Vec<serde_json::Value>,
}

impl RuntimeProxyRuntimeMemSuperSlimBenchCase {
    pub fn new(event_count: usize) -> Self {
        let event_count = event_count.max(16);
        let large_prompt = format!(
            "Inspect runtime proxy smart-context/token rewrite paths. {}",
            "large prompt fragment with repeated token pressure ".repeat(96)
        );
        let large_tool_output = (0..48)
            .map(|index| {
                format!(
                    "tool line {index:03}: crates/prodex-runtime-mem/src/lib.rs RuntimeMemRecallIntent psc:0123456789abcdef {}",
                    "diagnostic-output ".repeat(12)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut events = Vec::with_capacity(event_count);
        for index in 0..event_count {
            let event = match index % 4 {
                0 => serde_json::json!({
                    "id": format!("event-user-{index:03}"),
                    "payload": {
                        "type": "user_message",
                        "message": large_prompt,
                        "metadata": {
                            "prompt_summary": format!("summary {index:03} refs psc:0123456789abcdef")
                        }
                    }
                }),
                1 => serde_json::json!({
                    "id": format!("event-tool-{index:03}"),
                    "payload": {
                        "type": "function_call_output",
                        "call_id": format!("call-{index:03}"),
                        "output": large_tool_output,
                        "metadata": {
                            "summary": format!("tool summary {index:03} refs psc:fedcba9876543210"),
                            "artifact_ref": "psc:fedcba9876543210"
                        }
                    }
                }),
                2 => serde_json::json!({
                    "id": format!("event-assistant-{index:03}"),
                    "payload": {
                        "type": "agent_message",
                        "message": "assistant reasoning and answer ".repeat(160),
                        "summary": format!("assistant summary {index:03}")
                    }
                }),
                _ => serde_json::json!({
                    "id": format!("event-turn-{index:03}"),
                    "payload": {
                        "type": "turn_completed"
                    }
                }),
            };
            events.push(event);
        }
        Self { events }
    }

    pub fn shadow_token_heavy_events(&self) -> usize {
        prodex_runtime_mem::runtime_mem_super_slim_v2_shadow_codex_events(self.events.iter())
            .iter()
            .map(serde_json::Value::to_string)
            .map(|value| value.len())
            .sum()
    }
}
