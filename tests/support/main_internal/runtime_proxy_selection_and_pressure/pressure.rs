use super::*;

#[test]
fn runtime_proxy_default_worker_counts_stay_defensive() {
    assert_eq!(runtime_proxy_worker_count_default(1), 4);
    assert_eq!(runtime_proxy_worker_count_default(4), 4);
    assert_eq!(runtime_proxy_worker_count_default(8), 8);
    assert_eq!(runtime_proxy_worker_count_default(32), 12);

    assert_eq!(runtime_proxy_long_lived_worker_count_default(1), 8);
    assert_eq!(runtime_proxy_long_lived_worker_count_default(4), 8);
    assert_eq!(runtime_proxy_long_lived_worker_count_default(8), 16);
    assert_eq!(runtime_proxy_long_lived_worker_count_default(32), 24);

    assert_eq!(runtime_proxy_long_lived_queue_capacity(1), 128);
    assert_eq!(runtime_proxy_long_lived_queue_capacity(8), 128);
    assert_eq!(runtime_proxy_long_lived_queue_capacity(16), 128);
    assert_eq!(runtime_proxy_long_lived_queue_capacity(24), 192);
}

#[test]
fn runtime_proxy_async_worker_count_is_bounded() {
    let _low_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT", "1");
    let low = runtime_proxy_async_worker_count();
    assert!(
        (2..=8).contains(&low),
        "low async worker override should clamp into 2..=8, got {low}"
    );

    let _high_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT", "999");
    let high = runtime_proxy_async_worker_count();
    assert!(
        (2..=8).contains(&high),
        "high async worker override should clamp into 2..=8, got {high}"
    );
}

#[test]
fn runtime_proxy_async_worker_count_default_scales_with_parallelism() {
    assert_eq!(runtime_proxy_async_worker_count_default(1), 2);
    assert_eq!(runtime_proxy_async_worker_count_default(2), 2);
    assert_eq!(runtime_proxy_async_worker_count_default(4), 4);
    assert_eq!(runtime_proxy_async_worker_count_default(64), 4);
}

#[test]
fn runtime_proxy_default_active_limit_scales_with_long_lived_streams() {
    assert_eq!(runtime_proxy_active_request_limit_default(4, 8), 64);
    assert_eq!(runtime_proxy_active_request_limit_default(12, 24), 84);
    assert_eq!(runtime_proxy_active_request_limit_default(12, 32), 108);

    let lane_limits =
        runtime_proxy_lane_limits(runtime_proxy_active_request_limit_default(12, 24), 12, 24);
    assert!(lane_limits.responses > lane_limits.compact);
    assert!(lane_limits.responses > lane_limits.standard);
    assert!(lane_limits.responses >= lane_limits.websocket);
}

#[test]
fn runtime_buffered_response_parts_drop_requests_heap_trim_after_large_release() {
    reset_runtime_heap_trim_request_count();
    drop(RuntimeBufferedResponseParts {
        status: 200,
        headers: Vec::new(),
        body: vec![0_u8; RUNTIME_PROXY_HEAP_TRIM_MIN_RELEASE_BYTES].into(),
    });
    assert_eq!(runtime_heap_trim_request_count(), 1);

    reset_runtime_heap_trim_request_count();
    drop(RuntimeBufferedResponseParts {
        status: 200,
        headers: Vec::new(),
        body: vec![0_u8; RUNTIME_PROXY_HEAP_TRIM_MIN_RELEASE_BYTES / 2].into(),
    });
    assert_eq!(runtime_heap_trim_request_count(), 0);
}

#[test]
fn buffered_runtime_proxy_response_drop_requests_heap_trim_after_large_release() {
    reset_runtime_heap_trim_request_count();
    let response = build_runtime_proxy_response_from_parts(RuntimeBufferedResponseParts {
        status: 200,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: vec![0_u8; RUNTIME_PROXY_HEAP_TRIM_MIN_RELEASE_BYTES].into(),
    });
    drop(response);
    assert_eq!(runtime_heap_trim_request_count(), 1);
}

#[test]
fn runtime_profile_inflight_soft_limit_tightens_under_pressure() {
    let steady_responses = runtime_profile_inflight_soft_limit(RuntimeRouteKind::Responses, false);
    let pressured_responses =
        runtime_profile_inflight_soft_limit(RuntimeRouteKind::Responses, true);
    let pressured_compact = runtime_profile_inflight_soft_limit(RuntimeRouteKind::Compact, true);

    assert_eq!(
        steady_responses,
        runtime_proxy_profile_inflight_soft_limit().max(1)
    );
    assert!(pressured_responses >= 1);
    assert!(pressured_responses <= steady_responses);
    assert!(pressured_compact >= 1);
    assert!(pressured_compact <= pressured_responses);
}

#[test]
fn runtime_proxy_background_queue_pressure_is_route_aware() {
    assert!(!runtime_proxy_pressure_mode_for_route(
        RuntimeRouteKind::Responses,
        false,
        true,
    ));
    assert!(!runtime_proxy_pressure_mode_for_route(
        RuntimeRouteKind::Websocket,
        false,
        true,
    ));
    assert!(runtime_proxy_pressure_mode_for_route(
        RuntimeRouteKind::Compact,
        false,
        true,
    ));
    assert!(runtime_proxy_pressure_mode_for_route(
        RuntimeRouteKind::Standard,
        false,
        true,
    ));
    assert!(runtime_proxy_pressure_mode_for_route(
        RuntimeRouteKind::Responses,
        true,
        false,
    ));
}

#[test]
fn runtime_proxy_only_responses_lane_limit_marks_global_overload() {
    assert!(runtime_proxy_lane_limit_marks_global_overload(
        RuntimeRouteKind::Responses
    ));
    assert!(!runtime_proxy_lane_limit_marks_global_overload(
        RuntimeRouteKind::Compact
    ));
    assert!(!runtime_proxy_lane_limit_marks_global_overload(
        RuntimeRouteKind::Websocket
    ));
    assert!(!runtime_proxy_lane_limit_marks_global_overload(
        RuntimeRouteKind::Standard
    ));
}

#[test]
fn runtime_state_save_debounce_applies_to_hot_continuation_updates() {
    assert_eq!(
        runtime_state_save_debounce("profile_commit:main"),
        Duration::ZERO
    );
    assert!(
        runtime_state_save_debounce("session_id:main") > Duration::ZERO,
        "session id saves should be debounced"
    );
    assert!(
        runtime_state_save_debounce("response_ids:main") > Duration::ZERO,
        "response id saves should be debounced"
    );
    assert!(
        runtime_state_save_debounce("compact_session_touch:session-main") > Duration::ZERO,
        "compact lineage touches should be debounced"
    );
}

#[test]
fn runtime_state_save_reason_only_journals_owner_changes() {
    assert!(runtime_state_save_reason_requires_continuation_journal(
        "response_ids:main"
    ));
    assert!(runtime_state_save_reason_requires_continuation_journal(
        "turn_state:turn-main"
    ));
    assert!(runtime_state_save_reason_requires_continuation_journal(
        "session_id:session-main"
    ));
    assert!(runtime_state_save_reason_requires_continuation_journal(
        "compact_lineage:main"
    ));
    assert!(!runtime_state_save_reason_requires_continuation_journal(
        "response_touch:main"
    ));
    assert!(!runtime_state_save_reason_requires_continuation_journal(
        "turn_state_touch:main"
    ));
    assert!(!runtime_state_save_reason_requires_continuation_journal(
        "session_touch:main"
    ));
    assert!(!runtime_state_save_reason_requires_continuation_journal(
        "compact_session_touch:main"
    ));
}

#[derive(Debug)]
struct TestScheduledJob {
    ready_at: Instant,
}

impl RuntimeScheduledSaveJob for TestScheduledJob {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

#[test]
fn runtime_take_due_scheduled_jobs_waits_for_future_entries() {
    let now = Instant::now();
    let mut pending = BTreeMap::from([(
        "later".to_string(),
        TestScheduledJob {
            ready_at: now + Duration::from_millis(25),
        },
    )]);

    match runtime_take_due_scheduled_jobs(&mut pending, now) {
        RuntimeDueJobs::Wait(wait_for) => {
            assert!(wait_for >= Duration::from_millis(20));
            assert_eq!(pending.len(), 1);
        }
        RuntimeDueJobs::Due(_) => panic!("future work should not be returned early"),
    }
}

#[test]
fn runtime_take_due_scheduled_jobs_returns_only_ready_entries() {
    let now = Instant::now();
    let mut pending = BTreeMap::from([
        ("due".to_string(), TestScheduledJob { ready_at: now }),
        (
            "later".to_string(),
            TestScheduledJob {
                ready_at: now + Duration::from_millis(50),
            },
        ),
    ]);

    match runtime_take_due_scheduled_jobs(&mut pending, now) {
        RuntimeDueJobs::Due(due) => {
            assert_eq!(due.len(), 1);
            assert!(due.contains_key("due"));
            assert_eq!(pending.len(), 1);
            assert!(pending.contains_key("later"));
        }
        RuntimeDueJobs::Wait(_) => panic!("ready work should be returned immediately"),
    }
}

#[test]
fn runtime_soften_persisted_backoffs_for_startup_clamps_short_lived_penalties() {
    let now = Local::now().timestamp();
    let circuit_key = runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses);
    let mut backoffs = RuntimeProfileBackoffs {
        retry_backoff_until: BTreeMap::from([("retry".to_string(), now + 60)]),
        transport_backoff_until: BTreeMap::from([
            ("expired".to_string(), now - 1),
            (
                runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Responses),
                now + 60,
            ),
        ]),
        route_circuit_open_until: BTreeMap::from([
            ("expired".to_string(), now - 1),
            (circuit_key.clone(), now + 60),
        ]),
    };
    let profile_scores = BTreeMap::from([(
        runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
        RuntimeProfileHealth {
            score: RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 2,
            updated_at: now,
        },
    )]);

    runtime_soften_persisted_backoffs_for_startup(&mut backoffs, &profile_scores, now);

    assert_eq!(backoffs.retry_backoff_until.get("retry"), Some(&(now + 60)));
    assert!(!backoffs.transport_backoff_until.contains_key("expired"));
    assert_eq!(
        backoffs
            .transport_backoff_until
            .get(&runtime_profile_transport_backoff_key(
                "main",
                RuntimeRouteKind::Responses,
            )),
        Some(&now.saturating_add(RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS))
    );
    assert!(!backoffs.route_circuit_open_until.contains_key("expired"));
    assert_eq!(
        backoffs.route_circuit_open_until.get(&circuit_key),
        Some(
            &now.saturating_add(runtime_profile_circuit_half_open_probe_seconds(
                RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 2
            ))
        )
    );
}

#[test]
fn runtime_softened_backoffs_persist_after_proxy_startup() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
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
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let saved_now = Local::now().timestamp();
    save_runtime_profile_backoffs(
        &paths,
        &RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([("main".to_string(), saved_now + 600)]),
            transport_backoff_until: BTreeMap::from([(
                runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Responses),
                saved_now + 600,
            )]),
            route_circuit_open_until: BTreeMap::from([(
                runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses),
                saved_now + 600,
            )]),
        },
    )
    .expect("backoffs should save");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    wait_for_runtime_background_queues_idle();

    let loaded =
        load_runtime_profile_backoffs(&paths, &state.profiles).expect("backoffs should reload");
    let now = Local::now().timestamp();
    assert_eq!(
        loaded.retry_backoff_until.get("main"),
        Some(&(saved_now + 600))
    );
    assert!(
        loaded
            .transport_backoff_until
            .get(&runtime_profile_transport_backoff_key(
                "main",
                RuntimeRouteKind::Responses,
            ))
            .is_none_or(|until| *until <= now + RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS)
    );
    assert!(
        loaded
            .route_circuit_open_until
            .get(&runtime_profile_route_circuit_key(
                "main",
                RuntimeRouteKind::Responses
            ))
            .is_none_or(|until| *until <= now + RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS)
    );

    drop(proxy);
}

#[test]
fn runtime_state_save_accepts_legacy_backoffs_without_last_good_backup() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    create_codex_home_if_missing(&profile_home).expect("profile home should be created");

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
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let now = Local::now().timestamp();
    let transport_key = runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Responses);
    fs::write(
        runtime_backoffs_file_path(&paths),
        format!(
            concat!(
                "{{\n",
                "  \"retry_backoff_until\": {{\"main\": {}}},\n",
                "  \"transport_backoff_until\": {{\"{}\": {}}}\n",
                "}}\n"
            ),
            now + 600,
            transport_key,
            now + 300,
        ),
    )
    .expect("legacy raw runtime backoffs should be written");

    let runtime = RuntimeRotationState {
        paths: paths.clone(),
        state: state.clone(),
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
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    schedule_runtime_state_save(
        &shared,
        state.clone(),
        runtime_continuation_store_from_app_state(&state),
        BTreeMap::new(),
        BTreeMap::new(),
        RuntimeProfileBackoffs::default(),
        paths.clone(),
        "legacy_backoffs",
    );
    wait_for_runtime_background_queues_idle();

    let log =
        fs::read_to_string(&shared.log_path).expect("runtime state save log should be readable");
    assert!(
        log.contains("state_save_ok"),
        "legacy backoffs should not break runtime saves: {log}"
    );
    assert!(
        !log.contains("state_save_error"),
        "legacy backoffs should not emit state save errors: {log}"
    );

    let loaded =
        load_runtime_profile_backoffs(&paths, &state.profiles).expect("backoffs should reload");
    assert_eq!(loaded.retry_backoff_until.get("main"), Some(&(now + 600)));
    assert_eq!(
        loaded.transport_backoff_until.get(&transport_key),
        Some(&(now + 300))
    );
    assert!(
        loaded.route_circuit_open_until.is_empty(),
        "missing legacy route circuit data should default to empty"
    );
}

#[test]
fn runtime_fault_injection_consumes_budget() {
    let _guard = TestEnvVarGuard::set("PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE", "2");

    assert!(runtime_take_fault_injection(
        "PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE"
    ));
    assert!(runtime_take_fault_injection(
        "PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE"
    ));
    assert!(!runtime_take_fault_injection(
        "PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE"
    ));
}
