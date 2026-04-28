use super::*;

fn previous_response_affinity_test_shared(
    temp_dir: &TestDir,
    response_id: &str,
    now: i64,
    profile_health: BTreeMap<String, RuntimeProfileHealth>,
) -> RuntimeRotationProxyShared {
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            response_id.to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    RuntimeRotationProxyShared {
        upstream_no_proxy: false,
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
        active_request_limit: usize::MAX,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
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
            profile_health,
        })),
    }
}

#[test]
fn previous_response_affinity_release_requires_repeated_not_found() {
    let temp_dir = TestDir::isolated();
    let now = Local::now().timestamp();
    let shared =
        previous_response_affinity_test_shared(&temp_dir, "resp-main", now, BTreeMap::new());

    assert!(!release_runtime_previous_response_affinity(
        &shared,
        "main",
        Some("resp-main"),
        None,
        None,
        RuntimeRouteKind::Responses,
    )
    .expect("first not-found should defer hard release"));
    assert!(shared
        .runtime
        .lock()
        .expect("runtime lock")
        .state
        .response_profile_bindings
        .contains_key("resp-main"));

    assert!(release_runtime_previous_response_affinity(
        &shared,
        "main",
        Some("resp-main"),
        None,
        None,
        RuntimeRouteKind::Responses,
    )
    .expect("second not-found should release affinity"));
    assert!(!shared
        .runtime
        .lock()
        .expect("runtime lock")
        .state
        .response_profile_bindings
        .contains_key("resp-main"));
    assert_eq!(
        shared
            .runtime
            .lock()
            .expect("runtime lock")
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn previous_response_affinity_release_triggers_at_negative_cache_threshold() {
    let temp_dir = TestDir::isolated();
    let now = Local::now().timestamp();
    let response_id = "resp-main";
    let negative_cache_key = runtime_previous_response_negative_cache_key(
        response_id,
        "main",
        RuntimeRouteKind::Responses,
    );
    let seeded_failures = RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD - 1;
    let shared = previous_response_affinity_test_shared(
        &temp_dir,
        response_id,
        now,
        BTreeMap::from([(
            negative_cache_key.clone(),
            RuntimeProfileHealth {
                score: seeded_failures,
                updated_at: now,
            },
        )]),
    );

    assert!(release_runtime_previous_response_affinity(
        &shared,
        "main",
        Some(response_id),
        None,
        None,
        RuntimeRouteKind::Responses,
    )
    .expect("threshold-reaching not-found should release affinity"));

    let runtime = shared.runtime.lock().expect("runtime lock");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key(response_id),
        "binding should release once negative cache hits threshold"
    );
    assert_eq!(
        runtime
            .profile_health
            .get(&negative_cache_key)
            .map(|health| health.score),
        Some(RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD)
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .response
            .get(response_id)
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn previous_response_affinity_ignores_expired_negative_cache_until_threshold_rebuilds() {
    let temp_dir = TestDir::isolated();
    let now = Local::now().timestamp();
    let response_id = "resp-main";
    let negative_cache_key = runtime_previous_response_negative_cache_key(
        response_id,
        "main",
        RuntimeRouteKind::Responses,
    );
    let shared = previous_response_affinity_test_shared(
        &temp_dir,
        response_id,
        now,
        BTreeMap::from([(
            negative_cache_key.clone(),
            RuntimeProfileHealth {
                score: RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD - 1,
                updated_at: now - RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS - 1,
            },
        )]),
    );

    assert!(!release_runtime_previous_response_affinity(
        &shared,
        "main",
        Some(response_id),
        None,
        None,
        RuntimeRouteKind::Responses,
    )
    .expect("expired negative cache should not force early release"));

    {
        let runtime = shared.runtime.lock().expect("runtime lock");
        assert!(
            runtime
                .state
                .response_profile_bindings
                .contains_key(response_id),
            "expired failures should not release affinity on first fresh miss"
        );
        assert_eq!(
            runtime
                .profile_health
                .get(&negative_cache_key)
                .map(|health| health.score),
            Some(1)
        );
    }

    assert!(release_runtime_previous_response_affinity(
        &shared,
        "main",
        Some(response_id),
        None,
        None,
        RuntimeRouteKind::Responses,
    )
    .expect("freshly rebuilt threshold should still release on second miss"));

    let runtime = shared.runtime.lock().expect("runtime lock");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key(response_id),
        "binding should release after fresh misses rebuild threshold"
    );
    assert_eq!(
        runtime
            .profile_health
            .get(&negative_cache_key)
            .map(|health| health.score),
        Some(RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD)
    );
}

#[test]
fn previous_response_negative_cache_boundary_matrix_respects_threshold_and_expiry() {
    #[derive(Clone, Copy)]
    struct BoundaryCase {
        name: &'static str,
        seeded_score: Option<u32>,
        updated_at: i64,
        expect_release: bool,
        expect_score: u32,
        expect_binding_retained: bool,
    }

    let now = Local::now().timestamp();
    let cases = [
        BoundaryCase {
            name: "empty_cache_first_miss",
            seeded_score: None,
            updated_at: now,
            expect_release: false,
            expect_score: 1,
            expect_binding_retained: true,
        },
        BoundaryCase {
            name: "fresh_threshold_minus_one_releases",
            seeded_score: Some(RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD - 1),
            updated_at: now,
            expect_release: true,
            expect_score: RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD,
            expect_binding_retained: false,
        },
        BoundaryCase {
            name: "expired_threshold_minus_one_restarts_counter",
            seeded_score: Some(RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD - 1),
            updated_at: now - RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS - 1,
            expect_release: false,
            expect_score: 1,
            expect_binding_retained: true,
        },
    ];

    for route_kind in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Standard,
    ] {
        for case in cases {
            let temp_dir = TestDir::isolated();
            let response_id = format!(
                "resp-{}-{}",
                runtime_route_kind_label(route_kind),
                case.name
            );
            let negative_cache = case.seeded_score.map(|score| {
                (
                    runtime_previous_response_negative_cache_key(&response_id, "main", route_kind),
                    RuntimeProfileHealth {
                        score,
                        updated_at: case.updated_at,
                    },
                )
            });
            let shared = previous_response_affinity_test_shared(
                &temp_dir,
                &response_id,
                now,
                negative_cache.into_iter().collect(),
            );

            assert_eq!(
                release_runtime_previous_response_affinity(
                    &shared,
                    "main",
                    Some(&response_id),
                    None,
                    None,
                    route_kind,
                )
                .expect("boundary release should succeed"),
                case.expect_release,
                "route={} case={}",
                runtime_route_kind_label(route_kind),
                case.name
            );

            let runtime = shared.runtime.lock().expect("runtime lock");
            assert_eq!(
                runtime
                    .state
                    .response_profile_bindings
                    .contains_key(&response_id),
                case.expect_binding_retained,
                "route={} case={}",
                runtime_route_kind_label(route_kind),
                case.name
            );
            assert_eq!(
                runtime
                    .profile_health
                    .get(&runtime_previous_response_negative_cache_key(
                        &response_id,
                        "main",
                        route_kind,
                    ))
                    .map(|health| health.score),
                Some(case.expect_score),
                "route={} case={}",
                runtime_route_kind_label(route_kind),
                case.name
            );
        }
    }
}

#[test]
fn previous_response_negative_cache_keys_stay_route_scoped() {
    let now = Local::now().timestamp();
    for seeded_route in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Standard,
    ] {
        for release_route in [
            RuntimeRouteKind::Responses,
            RuntimeRouteKind::Websocket,
            RuntimeRouteKind::Compact,
            RuntimeRouteKind::Standard,
        ] {
            if seeded_route == release_route {
                continue;
            }

            let temp_dir = TestDir::isolated();
            let response_id = format!(
                "resp-{}-from-{}",
                runtime_route_kind_label(release_route),
                runtime_route_kind_label(seeded_route)
            );
            let seeded_key =
                runtime_previous_response_negative_cache_key(&response_id, "main", seeded_route);
            let release_key =
                runtime_previous_response_negative_cache_key(&response_id, "main", release_route);
            let shared = previous_response_affinity_test_shared(
                &temp_dir,
                &response_id,
                now,
                BTreeMap::from([(
                    seeded_key.clone(),
                    RuntimeProfileHealth {
                        score: RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD,
                        updated_at: now,
                    },
                )]),
            );

            assert!(
                !release_runtime_previous_response_affinity(
                    &shared,
                    "main",
                    Some(&response_id),
                    None,
                    None,
                    release_route,
                )
                .expect("route-scoped release should succeed"),
                "release route should ignore negative cache from a different route"
            );

            let runtime = shared.runtime.lock().expect("runtime lock");
            assert!(
                runtime
                    .state
                    .response_profile_bindings
                    .contains_key(&response_id),
                "route-scoped cache should not release affinity"
            );
            assert_eq!(
                runtime
                    .profile_health
                    .get(&seeded_key)
                    .map(|health| health.score),
                Some(RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD),
                "seeded route score should stay untouched"
            );
            assert_eq!(
                runtime
                    .profile_health
                    .get(&release_key)
                    .map(|health| health.score),
                Some(1),
                "release route should build its own counter from scratch"
            );
        }
    }
}
