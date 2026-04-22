#[test]
fn merge_runtime_usage_snapshots_keeps_newer_entries() {
    let now = Local::now().timestamp();
    let existing = BTreeMap::from([(
        "main".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: now - 20,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 70,
            five_hour_reset_at: now + 100,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 80,
            weekly_reset_at: now + 200,
        },
    )]);
    let incoming = BTreeMap::from([
        (
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now - 10,
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Critical,
                weekly_remaining_percent: 5,
                weekly_reset_at: now + 400,
            },
        ),
        (
            "stale".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now - 10,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 100,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 100,
                weekly_reset_at: now + 400,
            },
        ),
    ]);
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: PathBuf::from("/tmp/main"),
            managed: true,
            email: None,
            provider: ProfileProvider::Openai,
        },
    )]);

    let merged = merge_runtime_usage_snapshots(&existing, &incoming, &profiles);
    assert_eq!(merged.len(), 1);
    assert_eq!(
        merged
            .get("main")
            .expect("main snapshot should exist")
            .checked_at,
        now - 10
    );
    assert_eq!(
        merged
            .get("main")
            .expect("main snapshot should exist")
            .five_hour_status,
        RuntimeQuotaWindowStatus::Exhausted
    );
}

#[test]
fn runtime_profile_selection_jitter_is_deterministic_for_same_sequence() {
    let temp_dir = TestDir::new();
    let shared = RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        log_path: temp_dir.path.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(42)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
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
        })),
    };

    let first = runtime_profile_selection_jitter(&shared, "main", RuntimeRouteKind::Responses);
    let second = runtime_profile_selection_jitter(&shared, "main", RuntimeRouteKind::Responses);
    assert_eq!(first, second);
}

#[test]
fn runtime_profile_transport_health_penalty_weights_connect_failures_higher() {
    assert_eq!(
        runtime_profile_transport_health_penalty(RuntimeTransportFailureKind::ReadTimeout),
        RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
    );
    assert_eq!(
        runtime_profile_transport_health_penalty(RuntimeTransportFailureKind::ConnectTimeout),
        RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY
    );
    assert_eq!(
        runtime_profile_transport_health_penalty(RuntimeTransportFailureKind::BrokenPipe),
        RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
    );
}

#[test]
fn app_state_save_merges_existing_runtime_bindings() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let existing = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/main"),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/second"),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now - 20)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-existing".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 20,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-existing".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 20,
            },
        )]),
    };
    existing
        .save(&paths)
        .expect("initial state save should succeed");

    let desired = AppState {
        active_profile: Some("second".to_string()),
        profiles: existing.profiles.clone(),
        last_run_selected_at: BTreeMap::from([("second".to_string(), now - 10)]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    desired
        .save(&paths)
        .expect("merged state save should succeed");

    let loaded = AppState::load(&paths).expect("state should reload");
    assert_eq!(loaded.active_profile.as_deref(), Some("second"));
    assert_eq!(
        loaded
            .response_profile_bindings
            .get("resp-existing")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert_eq!(
        loaded
            .session_profile_bindings
            .get("sess-existing")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert_eq!(
        loaded.last_run_selected_at.get("main").copied(),
        Some(now - 20)
    );
    assert_eq!(
        loaded.last_run_selected_at.get("second").copied(),
        Some(now - 10)
    );
}

#[test]
fn app_state_housekeeping_prunes_stale_entries_on_save() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let stale_last_run = now - APP_STATE_LAST_RUN_RETENTION_SECONDS - 5;
    let stale_response_binding = now - 365 * 24 * 60 * 60;
    let stale_session_binding = now - APP_STATE_SESSION_BINDING_RETENTION_SECONDS - 5;
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::from([
            ("main".to_string(), now),
            ("ghost".to_string(), stale_last_run),
        ]),
        response_profile_bindings: BTreeMap::from([
            (
                "resp-fresh".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "resp-stale".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_response_binding,
                },
            ),
        ]),
        session_profile_bindings: BTreeMap::from([
            (
                "sess-fresh".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "sess-stale".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_session_binding,
                },
            ),
        ]),
    };
    state.save(&paths).expect("state should save");

    let loaded = AppState::load(&paths).expect("state should reload");
    let raw = fs::read_to_string(&paths.state_file).expect("state file should be readable");
    assert!(loaded.last_run_selected_at.contains_key("main"));
    assert!(!loaded.last_run_selected_at.contains_key("ghost"));
    assert!(loaded.response_profile_bindings.contains_key("resp-fresh"));
    assert!(loaded.response_profile_bindings.contains_key("resp-stale"));
    assert!(loaded.session_profile_bindings.contains_key("sess-fresh"));
    assert!(!loaded.session_profile_bindings.contains_key("sess-stale"));
    assert!(raw.contains("resp-fresh"));
    assert!(raw.contains("resp-stale"));
    assert!(!raw.contains("sess-stale"));
}

#[test]
fn app_state_load_compacts_stale_entries_in_memory() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(
        paths
            .state_file
            .parent()
            .expect("state file should have a parent"),
    )
    .expect("state dir should exist");
    let now = Local::now().timestamp();
    let stale_last_run = now - APP_STATE_LAST_RUN_RETENTION_SECONDS - 5;
    let stale_session_binding = now - APP_STATE_SESSION_BINDING_RETENTION_SECONDS - 5;
    let stale_response_binding = now - 365 * 24 * 60 * 60;
    let raw = serde_json::json!({
        "active_profile": "main",
        "profiles": {
            "main": {
                "codex_home": temp_dir.path.join("homes/main"),
                "managed": true,
                "email": "main@example.com"
            }
        },
        "last_run_selected_at": {
            "main": now,
            "ghost": stale_last_run
        },
        "response_profile_bindings": {
            "resp-stale": {
                "profile_name": "main",
                "bound_at": stale_response_binding
            }
        },
        "session_profile_bindings": {
            "sess-stale": {
                "profile_name": "main",
                "bound_at": stale_session_binding
            }
        }
    });
    fs::write(
        &paths.state_file,
        serde_json::to_string_pretty(&raw).expect("raw json should serialize"),
    )
    .expect("raw state should write");

    let loaded = AppState::load(&paths).expect("state should load");
    assert_eq!(loaded.active_profile.as_deref(), Some("main"));
    assert!(loaded.last_run_selected_at.contains_key("main"));
    assert!(!loaded.last_run_selected_at.contains_key("ghost"));
    assert!(loaded.response_profile_bindings.contains_key("resp-stale"));
    assert!(loaded.session_profile_bindings.is_empty());
}

#[test]
fn app_state_response_bindings_are_not_pruned_just_for_size() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let mut response_profile_bindings = BTreeMap::new();
    for index in 0..(RESPONSE_PROFILE_BINDING_LIMIT + 3) {
        response_profile_bindings.insert(
            format!("resp-{index:06}-{}", "x".repeat(64)),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now + index as i64,
            },
        );
    }
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings,
        session_profile_bindings: BTreeMap::new(),
    };

    let compacted = compact_app_state(state, now);
    assert_eq!(
        compacted.response_profile_bindings.len(),
        RESPONSE_PROFILE_BINDING_LIMIT + 3
    );
    assert!(compacted.response_profile_bindings.contains_key(&format!(
        "resp-{:06}-{}",
        0,
        "x".repeat(64)
    )));
}

#[test]
fn runtime_sidecar_housekeeping_prunes_stale_entries() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let stale = now - RUNTIME_SCORE_RETENTION_SECONDS - 5;

    let scores = compact_runtime_profile_scores(
        BTreeMap::from([
            (
                runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
                RuntimeProfileHealth {
                    score: 4,
                    updated_at: now,
                },
            ),
            (
                runtime_profile_route_bad_pairing_key("main", RuntimeRouteKind::Compact),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: stale,
                },
            ),
        ]),
        &profiles,
        now,
    );
    assert!(scores.contains_key(&runtime_profile_route_health_key(
        "main",
        RuntimeRouteKind::Responses
    )));
    assert!(!scores.contains_key(&runtime_profile_route_bad_pairing_key(
        "main",
        RuntimeRouteKind::Compact
    )));

    let snapshots = compact_runtime_usage_snapshots(
        BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 90,
                    five_hour_reset_at: now + 300,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 95,
                    weekly_reset_at: now + 600,
                },
            ),
            (
                "ghost".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: stale,
                    five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                    five_hour_remaining_percent: 0,
                    five_hour_reset_at: now + 300,
                    weekly_status: RuntimeQuotaWindowStatus::Exhausted,
                    weekly_remaining_percent: 0,
                    weekly_reset_at: now + 600,
                },
            ),
        ]),
        &profiles,
        now,
    );
    assert!(snapshots.contains_key("main"));
    assert!(!snapshots.contains_key("ghost"));

    let backoffs = compact_runtime_profile_backoffs(
        RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([
                ("main".to_string(), now + 60),
                ("ghost".to_string(), now + 60),
            ]),
            transport_backoff_until: BTreeMap::from([(
                runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Responses),
                now - 1,
            )]),
            route_circuit_open_until: BTreeMap::from([
                (
                    runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses),
                    now + 60,
                ),
                (
                    runtime_profile_route_circuit_key("ghost", RuntimeRouteKind::Responses),
                    now + 60,
                ),
            ]),
        },
        &profiles,
        now,
    );
    assert!(backoffs.retry_backoff_until.contains_key("main"));
    assert!(!backoffs.retry_backoff_until.contains_key("ghost"));
    assert!(backoffs.transport_backoff_until.is_empty());
    assert!(
        backoffs
            .route_circuit_open_until
            .contains_key(&runtime_profile_route_circuit_key(
                "main",
                RuntimeRouteKind::Responses
            ))
    );
    assert!(
        !backoffs
            .route_circuit_open_until
            .contains_key(&runtime_profile_route_circuit_key(
                "ghost",
                RuntimeRouteKind::Responses
            ))
    );
}

#[test]
fn runtime_log_housekeeping_prunes_old_logs_and_stale_pointer() {
    let temp_dir = TestDir::new();
    let old_one = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-1.log"));
    let old_two = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-2.log"));
    let keep_one = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-3.log"));
    let keep_two = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-4.log"));
    let keep_three = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-5.log"));
    let keep_four = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-6.log"));
    fs::write(&old_one, "old").expect("old log should write");
    fs::write(&old_two, "old").expect("old log should write");
    fs::write(&keep_one, "keep").expect("keep log should write");
    fs::write(&keep_two, "keep").expect("keep log should write");
    fs::write(&keep_three, "keep").expect("keep log should write");
    fs::write(&keep_four, "keep").expect("keep log should write");

    let pointer = temp_dir.path.join(RUNTIME_PROXY_LATEST_LOG_POINTER);
    fs::write(
        &pointer,
        format!("{}\n", temp_dir.path.join("missing.log").display()),
    )
    .expect("pointer should write");

    cleanup_runtime_proxy_logs_in_dir(&temp_dir.path, SystemTime::now());
    cleanup_runtime_proxy_latest_pointer(&pointer);

    assert!(!old_one.exists());
    assert!(!old_two.exists());
    assert!(keep_one.exists());
    assert!(keep_two.exists());
    assert!(keep_three.exists());
    assert!(keep_four.exists());
    assert!(!pointer.exists());
}

#[test]
fn stale_login_dir_housekeeping_removes_old_temp_login_homes() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    let stale_login = paths.root.join(".login-123-1-0");
    fs::create_dir_all(&stale_login).expect("stale login dir should exist");

    let simulated_now = SystemTime::now()
        .checked_add(Duration::from_secs(
            (PROD_EX_TMP_LOGIN_RETENTION_SECONDS + 5).max(1) as u64,
        ))
        .expect("simulated clock should be valid");
    cleanup_stale_login_dirs_at(&paths, simulated_now);

    assert!(!stale_login.exists());
}

#[test]
fn perform_prodex_cleanup_removes_safe_local_artifacts() {
    let temp_dir = TestDir::new();
    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    fs::create_dir_all(&runtime_log_dir).expect("runtime log dir should exist");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");

    for index in 0..=RUNTIME_PROXY_LOG_RETENTION_COUNT {
        let path = runtime_log_dir.join(format!(
            "{RUNTIME_PROXY_LOG_FILE_PREFIX}-cleanup-{index}.log"
        ));
        fs::write(path, "log").expect("runtime log should write");
    }
    let pointer = runtime_log_dir.join(RUNTIME_PROXY_LATEST_LOG_POINTER);
    fs::write(
        &pointer,
        format!("{}\n", runtime_log_dir.join("missing.log").display()),
    )
    .expect("pointer should write");

    let stale_login = paths.root.join(".login-123-1-0");
    fs::create_dir_all(&stale_login).expect("stale login dir should exist");

    let transient_root_files = [
        runtime_scores_file_path(&paths),
        runtime_scores_last_good_file_path(&paths),
        runtime_usage_snapshots_file_path(&paths),
        runtime_usage_snapshots_last_good_file_path(&paths),
        runtime_backoffs_file_path(&paths),
        runtime_backoffs_last_good_file_path(&paths),
        update_check_cache_file_path(&paths),
    ];
    for path in &transient_root_files {
        fs::write(path, "{}").expect("transient root file should write");
    }

    let stale_root_temp = paths.root.join("runtime-backoffs.json.999999999.1.0.tmp");
    fs::write(&stale_root_temp, "tmp").expect("stale root temp should write");

    let tracked = paths.managed_profiles_root.join("tracked");
    fs::create_dir_all(&tracked).expect("tracked dir should exist");
    fs::write(tracked.join("auth.json"), "{}").expect("tracked auth should be written");

    let orphan = paths.managed_profiles_root.join("orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan auth should be written");

    let state = AppState {
        active_profile: Some("tracked".to_string()),
        profiles: BTreeMap::from([(
            "tracked".to_string(),
            ProfileEntry {
                codex_home: tracked.clone(),
                managed: true,
                email: Some("tracked@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    let stale_broker_key = "cleanup-stale";
    save_runtime_broker_registry(
        &paths,
        stale_broker_key,
        &RuntimeBrokerRegistry {
            pid: 999_999_999,
            listen_addr: "127.0.0.1:1".to_string(),
            started_at: 1,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "tracked".to_string(),
            instance_token: "stale-instance".to_string(),
            admin_token: "stale-admin".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
            openai_mount_path: None,
        },
    )
    .expect("stale runtime broker registry should save");
    let stale_lease_dir = runtime_broker_lease_dir(&paths, stale_broker_key);
    fs::create_dir_all(&stale_lease_dir).expect("stale lease dir should exist");
    let stale_lease = stale_lease_dir.join("999999999-stale.lease");
    let live_lease = stale_lease_dir.join(format!("{}-live.lease", std::process::id()));
    fs::write(&stale_lease, "stale").expect("stale lease should write");
    fs::write(&live_lease, "live").expect("live lease should write");

    let lease_only_key = "cleanup-lease-only";
    let lease_only_dir = runtime_broker_lease_dir(&paths, lease_only_key);
    fs::create_dir_all(&lease_only_dir).expect("lease-only dir should exist");
    let lease_only_stale = lease_only_dir.join("999999998-stale.lease");
    fs::write(&lease_only_stale, "stale").expect("lease-only stale lease should write");

    let future_offset_seconds = (ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS
        .max(PROD_EX_TMP_LOGIN_RETENTION_SECONDS)
        + 5) as u64;
    let simulated_now = SystemTime::now()
        .checked_add(Duration::from_secs(future_offset_seconds))
        .expect("simulated clock should be valid");
    let summary =
        perform_prodex_cleanup_at(&paths, &state, &runtime_log_dir, &pointer, simulated_now)
            .expect("cleanup should succeed");

    let log_count = RUNTIME_PROXY_LOG_RETENTION_COUNT + 1;
    let expected_runtime_logs_removed =
        if future_offset_seconds as i64 >= RUNTIME_PROXY_LOG_RETENTION_SECONDS {
            log_count
        } else {
            1
        };
    assert_eq!(summary.runtime_logs_removed, expected_runtime_logs_removed);
    assert_eq!(summary.stale_runtime_log_pointer_removed, 1);
    assert_eq!(summary.stale_login_dirs_removed, 1);
    assert_eq!(summary.orphan_managed_profile_dirs_removed, 1);
    assert_eq!(
        summary.transient_root_files_removed,
        transient_root_files.len()
    );
    assert_eq!(summary.stale_root_temp_files_removed, 1);
    assert_eq!(summary.dead_runtime_broker_leases_removed, 2);
    assert_eq!(summary.dead_runtime_broker_registries_removed, 1);
    assert_eq!(
        summary.total_removed(),
        expected_runtime_logs_removed + transient_root_files.len() + 7
    );

    assert!(!pointer.exists(), "stale runtime pointer should be removed");
    assert!(!stale_login.exists(), "stale login dir should be removed");
    for path in &transient_root_files {
        assert!(
            !path.exists(),
            "transient root file {} should be removed",
            path.display()
        );
    }
    assert!(
        !stale_root_temp.exists(),
        "stale root temp file should be removed"
    );
    assert!(!orphan.exists(), "orphan managed dir should be removed");
    assert!(tracked.exists(), "tracked managed dir should remain");
    assert_eq!(
        prodex_runtime_log_paths_in_dir(&runtime_log_dir).len(),
        log_count.saturating_sub(expected_runtime_logs_removed)
    );
    assert!(
        !runtime_broker_registry_file_path(&paths, stale_broker_key).exists(),
        "stale runtime broker registry should be removed"
    );
    assert!(
        !runtime_broker_registry_last_good_file_path(&paths, stale_broker_key).exists(),
        "stale runtime broker registry backup should be removed"
    );
    assert!(!stale_lease.exists(), "dead lease should be removed");
    assert!(
        !lease_only_stale.exists(),
        "dead lease without registry should be removed"
    );
    assert!(live_lease.exists(), "live lease should remain");
}

#[test]
fn perform_prodex_cleanup_prunes_chat_history_older_than_one_week() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("prodex/.codex"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.shared_codex_root).expect("shared codex root should exist");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");

    let now = Local
        .with_ymd_and_hms(2026, 4, 20, 12, 0, 0)
        .single()
        .expect("fixed timestamp should be valid")
        .timestamp();
    let old_ts = now - (8 * 24 * 60 * 60);
    let recent_ts = now - (2 * 24 * 60 * 60);
    fs::write(
        paths.shared_codex_root.join("history.jsonl"),
        format!(
            "{{\"session_id\":\"old\",\"ts\":{old_ts},\"text\":\"old\"}}\n\
             {{\"session_id\":\"recent\",\"ts\":{recent_ts},\"text\":\"recent\"}}\n\
             {{\"session_id\":\"unstamped\",\"text\":\"keep\"}}\n"
        ),
    )
    .expect("shared codex history should write");

    let old_session_dir = paths.shared_codex_root.join("sessions/2026/04/10");
    let recent_session_dir = paths.shared_codex_root.join("sessions/2026/04/18");
    fs::create_dir_all(&old_session_dir).expect("old codex session dir should exist");
    fs::create_dir_all(&recent_session_dir).expect("recent codex session dir should exist");
    fs::write(old_session_dir.join("old.jsonl"), "{\"old\":true}\n")
        .expect("old codex session should write");
    fs::write(
        recent_session_dir.join("recent.jsonl"),
        "{\"recent\":true}\n",
    )
    .expect("recent codex session should write");

    let shared_claude_project =
        runtime_proxy_shared_claude_config_dir(&paths).join("projects/workspace");
    fs::create_dir_all(&shared_claude_project).expect("shared claude project should exist");
    fs::write(
        shared_claude_project.join("session.jsonl"),
        format!(
            "{{\"timestamp\":\"{}\",\"message\":\"old\"}}\n\
             {{\"timestamp\":\"{}\",\"message\":\"recent\"}}\n",
            Local
                .timestamp_opt(old_ts, 0)
                .single()
                .unwrap()
                .to_rfc3339(),
            Local
                .timestamp_opt(recent_ts, 0)
                .single()
                .unwrap()
                .to_rfc3339()
        ),
    )
    .expect("shared claude history should write");

    let profile_home = paths.managed_profiles_root.join("main");
    let profile_old_session_dir = profile_home.join("sessions/2026/04/09");
    fs::create_dir_all(&profile_old_session_dir).expect("profile old session dir should exist");
    fs::write(
        profile_old_session_dir.join("profile-old.json"),
        "{\"old\":true}\n",
    )
    .expect("profile old session should write");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    fs::create_dir_all(&runtime_log_dir).expect("runtime log dir should exist");
    let pointer = runtime_log_dir.join(RUNTIME_PROXY_LATEST_LOG_POINTER);
    let simulated_now = UNIX_EPOCH
        .checked_add(Duration::from_secs(now as u64))
        .expect("simulated time should be valid");
    let summary =
        perform_prodex_cleanup_at(&paths, &state, &runtime_log_dir, &pointer, simulated_now)
            .expect("cleanup should succeed");

    assert_eq!(summary.chat_history_entries_removed, 4);
    let codex_history = fs::read_to_string(paths.shared_codex_root.join("history.jsonl"))
        .expect("shared codex history should remain");
    assert!(!codex_history.contains("\"session_id\":\"old\""));
    assert!(codex_history.contains("\"session_id\":\"recent\""));
    assert!(codex_history.contains("\"session_id\":\"unstamped\""));
    assert!(
        !old_session_dir.join("old.jsonl").exists(),
        "old dated codex session should be removed"
    );
    assert!(
        recent_session_dir.join("recent.jsonl").exists(),
        "recent dated codex session should remain"
    );
    assert!(
        !profile_old_session_dir.join("profile-old.json").exists(),
        "old managed profile session should be removed"
    );

    let claude_history = fs::read_to_string(shared_claude_project.join("session.jsonl"))
        .expect("shared claude history should remain");
    assert!(!claude_history.contains("\"message\":\"old\""));
    assert!(claude_history.contains("\"message\":\"recent\""));
}

#[test]
fn perform_prodex_cleanup_deduplicates_profiles_by_email() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");

    let primary_home = paths.managed_profiles_root.join("primary");
    let duplicate_home = paths.managed_profiles_root.join("duplicate");
    fs::create_dir_all(&primary_home).expect("primary home should exist");
    fs::create_dir_all(&duplicate_home).expect("duplicate home should exist");

    let mut state = AppState {
        active_profile: Some("duplicate".to_string()),
        profiles: BTreeMap::from([
            (
                "primary".to_string(),
                ProfileEntry {
                    codex_home: primary_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "duplicate".to_string(),
                ProfileEntry {
                    codex_home: duplicate_home.clone(),
                    managed: true,
                    email: Some("Main@Example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("primary".to_string(), 10),
            ("duplicate".to_string(), 5),
        ]),
        response_profile_bindings: BTreeMap::from([(
            "resp-1".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 11,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-1".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 12,
            },
        )]),
    };
    state.save(&paths).expect("state should save");

    let continuations = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-2".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 13,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-2".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 14,
            },
        )]),
        turn_state_bindings: BTreeMap::from([(
            "turn-2".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 15,
            },
        )]),
        session_id_bindings: BTreeMap::from([(
            "sid-2".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 16,
            },
        )]),
        statuses: RuntimeContinuationStatuses::default(),
    };
    save_runtime_continuations_for_profiles(&paths, &continuations, &state.profiles)
        .expect("continuations should save");
    save_runtime_continuation_journal_for_profiles(&paths, &continuations, &state.profiles, 123)
        .expect("continuation journal should save");

    let summary = perform_prodex_cleanup(&paths, &mut state).expect("cleanup should succeed");

    assert_eq!(summary.duplicate_profiles_removed, 1);
    assert_eq!(summary.duplicate_managed_profile_homes_removed, 1);
    assert_eq!(state.active_profile.as_deref(), Some("duplicate"));
    assert_eq!(state.profiles.len(), 1);
    assert!(state.profiles.contains_key("duplicate"));
    assert_eq!(state.last_run_selected_at.get("duplicate"), Some(&10));
    assert_eq!(
        state.response_profile_bindings["resp-1"].profile_name,
        "duplicate"
    );
    assert_eq!(
        state.session_profile_bindings["sess-1"].profile_name,
        "duplicate"
    );
    assert!(
        !primary_home.exists(),
        "duplicate managed home should be removed"
    );
    assert!(
        duplicate_home.exists(),
        "canonical managed home should remain"
    );

    let saved_state = AppState::load(&paths).expect("saved state should load");
    assert_eq!(saved_state.active_profile.as_deref(), Some("duplicate"));
    assert!(saved_state.profiles.contains_key("duplicate"));
    assert!(!saved_state.profiles.contains_key("primary"));

    let restored_continuations = load_runtime_continuations_with_recovery(&paths, &state.profiles)
        .expect("continuations should load")
        .value;
    assert_eq!(
        restored_continuations.response_profile_bindings["resp-2"].profile_name,
        "duplicate"
    );
    assert_eq!(
        restored_continuations.session_profile_bindings["sess-2"].profile_name,
        "duplicate"
    );
    assert_eq!(
        restored_continuations.turn_state_bindings["turn-2"].profile_name,
        "duplicate"
    );
    assert_eq!(
        restored_continuations.session_id_bindings["sid-2"].profile_name,
        "duplicate"
    );

    let restored_journal = load_runtime_continuation_journal_with_recovery(&paths, &state.profiles)
        .expect("continuation journal should load")
        .value;
    assert_eq!(
        restored_journal.continuations.response_profile_bindings["resp-2"].profile_name,
        "duplicate"
    );
}

#[test]
fn runtime_state_snapshot_save_preserves_concurrent_profiles() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let existing = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/main"),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/second"),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::from([("second".to_string(), now - 20)]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    existing
        .save(&paths)
        .expect("initial state save should succeed");

    let snapshot = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now - 10)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
    };
    let revision = AtomicU64::new(1);
    assert!(
        save_runtime_state_snapshot_if_latest(
            &paths,
            &snapshot,
            &runtime_continuation_store_from_app_state(&snapshot),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &RuntimeProfileBackoffs::default(),
            1,
            &revision,
        )
        .expect("runtime snapshot save should succeed")
    );

    let loaded = AppState::load(&paths).expect("state should reload");
    assert_eq!(loaded.active_profile.as_deref(), Some("main"));
    assert!(loaded.profiles.contains_key("second"));
    assert_eq!(
        loaded
            .response_profile_bindings
            .get("resp-main")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert_eq!(
        loaded
            .session_profile_bindings
            .get("sess-main")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert_eq!(
        loaded.last_run_selected_at.get("second").copied(),
        Some(now - 20)
    );
    assert_eq!(
        loaded.last_run_selected_at.get("main").copied(),
        Some(now - 10)
    );
}

#[test]
fn runtime_state_save_scheduler_persists_latest_snapshot() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profiles = BTreeMap::from([
        (
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        ),
        (
            "second".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/second"),
                managed: true,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        ),
    ]);
    let shared = RuntimeRotationProxyShared {
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
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: paths.clone(),
            state: AppState::default(),
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
        })),
    };

    let first_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now - 20)]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    schedule_runtime_state_save(
        &shared,
        first_state.clone(),
        runtime_continuation_store_from_app_state(&first_state),
        BTreeMap::from([(
            runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                updated_at: now - 20,
            },
        )]),
        BTreeMap::new(),
        RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([("main".to_string(), now + 60)]),
            transport_backoff_until: BTreeMap::new(),
            route_circuit_open_until: BTreeMap::new(),
        },
        paths.clone(),
        "first",
    );
    let second_state = AppState {
        active_profile: Some("second".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("second".to_string(), now - 10)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now - 10,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now - 10,
            },
        )]),
    };
    schedule_runtime_state_save(
        &shared,
        second_state.clone(),
        runtime_continuation_store_from_app_state(&second_state),
        BTreeMap::from([(
            runtime_profile_route_health_key("second", RuntimeRouteKind::Compact),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                updated_at: now - 10,
            },
        )]),
        BTreeMap::new(),
        RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::new(),
            transport_backoff_until: BTreeMap::from([(
                runtime_profile_transport_backoff_key("second", RuntimeRouteKind::Responses),
                now + 120,
            )]),
            route_circuit_open_until: BTreeMap::new(),
        },
        paths.clone(),
        "second",
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
            && state
                .response_profile_bindings
                .get("resp-second")
                .is_some_and(|binding| binding.profile_name == "second")
            && state
                .session_profile_bindings
                .get("sess-second")
                .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
    assert_eq!(
        persisted.last_run_selected_at.get("second").copied(),
        Some(now - 10)
    );
    assert_eq!(
        persisted
            .session_profile_bindings
            .get("sess-second")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
    let persisted_scores =
        load_runtime_profile_scores(&paths, &profiles).expect("runtime scores should reload");
    let persisted_backoffs =
        load_runtime_profile_backoffs(&paths, &profiles).expect("runtime backoffs should reload");
    assert!(
        persisted_scores.contains_key(&runtime_profile_route_health_key(
            "second",
            RuntimeRouteKind::Compact
        )),
        "latest queued runtime scores should persist alongside state"
    );
    assert!(
        persisted_backoffs
            .transport_backoff_until
            .get(&runtime_profile_transport_backoff_key(
                "second",
                RuntimeRouteKind::Responses,
            ))
            .is_some_and(|until| *until > Local::now().timestamp())
    );
}

#[test]
fn runtime_state_save_sections_follow_dirty_reason_scope() {
    assert_eq!(
        runtime_state_save_sections_for_reason("usage_snapshot:main"),
        RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::None,
            continuations: false,
            profile_scores: false,
            usage_snapshots: true,
            backoffs: true,
        }
    );
    assert_eq!(
        runtime_state_save_sections_for_reason("response_ids:main"),
        RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::Core,
            continuations: true,
            profile_scores: true,
            usage_snapshots: false,
            backoffs: false,
        }
    );
    assert_eq!(
        runtime_state_save_sections_for_reason("profile_commit:second"),
        RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::Core,
            continuations: false,
            profile_scores: true,
            usage_snapshots: false,
            backoffs: true,
        }
    );
    assert_eq!(
        runtime_state_save_sections_for_reason("startup_audit"),
        RuntimeStateSaveSections::full()
    );
}

#[test]
fn runtime_state_selected_snapshot_preserves_unselected_sections() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profiles = BTreeMap::from([
        (
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        ),
        (
            "second".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/second"),
                managed: true,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        ),
    ]);
    let initial_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now - 20)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 20,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 20,
            },
        )]),
    };
    initial_state
        .save(&paths)
        .expect("initial state should save");
    let initial_scores = BTreeMap::from([(
        runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
        RuntimeProfileHealth {
            score: 3,
            updated_at: now - 20,
        },
    )]);
    save_runtime_profile_scores_for_profiles(&paths, &initial_scores, &profiles)
        .expect("initial scores should save");
    let initial_usage = BTreeMap::from([(
        "main".to_string(),
        ready_runtime_usage_snapshot(now - 20, 80),
    )]);
    save_runtime_usage_snapshots_for_profiles(&paths, &initial_usage, &profiles)
        .expect("initial usage snapshots should save");
    let initial_backoffs = RuntimeProfileBackoffs {
        retry_backoff_until: BTreeMap::from([("main".to_string(), now + 60)]),
        transport_backoff_until: BTreeMap::new(),
        route_circuit_open_until: BTreeMap::new(),
    };
    save_runtime_profile_backoffs_for_profiles(&paths, &initial_backoffs, &profiles)
        .expect("initial backoffs should save");

    let selected_state = AppState {
        active_profile: Some("second".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("second".to_string(), now - 10)]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let selected_usage = BTreeMap::from([(
        "second".to_string(),
        ready_runtime_usage_snapshot(now - 10, 55),
    )]);
    let selected = RuntimeStateSaveSelectedSnapshot {
        paths: paths.clone(),
        state: Some(selected_state),
        profiles: None,
        continuations: None,
        profile_scores: None,
        usage_snapshots: Some(selected_usage),
        backoffs: None,
    };
    let revision = AtomicU64::new(1);
    assert!(
        save_runtime_state_selected_snapshot_if_latest(&selected, 1, &revision)
            .expect("selected state save should succeed")
    );

    let loaded = AppState::load(&paths).expect("state should reload");
    assert_eq!(loaded.active_profile.as_deref(), Some("second"));
    assert_eq!(
        loaded.last_run_selected_at.get("main").copied(),
        Some(now - 20)
    );
    assert_eq!(
        loaded.last_run_selected_at.get("second").copied(),
        Some(now - 10)
    );
    assert!(
        loaded.response_profile_bindings.contains_key("resp-main"),
        "core selected state save must not drop existing response affinity"
    );
    assert!(
        loaded.session_profile_bindings.contains_key("sess-main"),
        "core selected state save must not drop existing session affinity"
    );
    let loaded_scores =
        load_runtime_profile_scores(&paths, &profiles).expect("scores should reload");
    assert_eq!(loaded_scores.len(), initial_scores.len());
    assert_eq!(
        loaded_scores
            .get(&runtime_profile_route_health_key(
                "main",
                RuntimeRouteKind::Responses
            ))
            .map(|score| (score.score, score.updated_at)),
        Some((3, now - 20))
    );
    let loaded_backoffs =
        load_runtime_profile_backoffs(&paths, &profiles).expect("backoffs should reload");
    assert_eq!(
        loaded_backoffs.retry_backoff_until,
        initial_backoffs.retry_backoff_until
    );
    assert_eq!(
        loaded_backoffs.transport_backoff_until,
        initial_backoffs.transport_backoff_until
    );
    assert_eq!(
        loaded_backoffs.route_circuit_open_until,
        initial_backoffs.route_circuit_open_until
    );
    let loaded_usage =
        load_runtime_usage_snapshots(&paths, &profiles).expect("usage snapshots should reload");
    assert!(loaded_usage.contains_key("main"));
    assert!(loaded_usage.contains_key("second"));
}

#[test]
fn runtime_backoffs_load_legacy_last_good_backup_when_primary_is_invalid() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);

    let now = Local::now().timestamp();
    let legacy_backup = serde_json::json!({
        "retry_backoff_until": {
            "main": now + 600
        },
        "transport_backoff_until": {}
    });
    fs::write(
        runtime_backoffs_last_good_file_path(&paths),
        serde_json::to_string_pretty(&legacy_backup)
            .expect("legacy backup should serialize cleanly"),
    )
    .expect("legacy backup should be writable");
    fs::write(runtime_backoffs_file_path(&paths), "{ not valid json")
        .expect("broken primary backoffs should be writable");

    let loaded = load_runtime_profile_backoffs_with_recovery(&paths, &profiles)
        .expect("legacy backup recovery should succeed");

    assert!(loaded.recovered_from_backup);
    assert_eq!(
        loaded.value.retry_backoff_until.get("main"),
        Some(&(now + 600))
    );
    assert!(loaded.value.transport_backoff_until.is_empty());
    assert!(loaded.value.route_circuit_open_until.is_empty());
}

#[test]
fn runtime_state_snapshot_save_returns_error_on_injected_failure() {
    let temp_dir = TestDir::new();
    let _guard = TestEnvVarGuard::set("PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE", "1");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let snapshot = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let latest_revision = AtomicU64::new(1);

    let err = save_runtime_state_snapshot_if_latest(
        &paths,
        &snapshot,
        &runtime_continuation_store_from_app_state(&snapshot),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &RuntimeProfileBackoffs::default(),
        1,
        &latest_revision,
    )
    .expect_err("injected save failure should bubble up");
    assert!(
        err.to_string()
            .contains("injected runtime state save failure")
    );
}

#[test]
fn app_state_load_uses_last_good_backup_when_primary_is_invalid() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-1".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let backup_json =
        serde_json::to_string_pretty(&state).expect("state backup should serialize cleanly");
    fs::write(state_last_good_file_path(&paths), backup_json)
        .expect("last-good backup should be writable");
    fs::write(&paths.state_file, "{ not valid json").expect("broken primary state should write");

    let loaded = AppState::load_with_recovery(&paths).expect("backup recovery should succeed");

    assert!(loaded.recovered_from_backup);
    assert_eq!(loaded.value.active_profile.as_deref(), Some("main"));
    assert_eq!(
        loaded
            .value
            .response_profile_bindings
            .get("resp-1")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
}

#[test]
fn runtime_continuations_load_legacy_last_good_backup_when_primary_is_invalid() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let backup_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-legacy".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    fs::write(
        runtime_continuations_last_good_file_path(&paths),
        serde_json::to_string_pretty(&backup_store)
            .expect("legacy continuation backup should serialize"),
    )
    .expect("legacy continuation backup should write");
    fs::write(runtime_continuations_file_path(&paths), "{ not valid json")
        .expect("broken continuation primary should write");

    let loaded = load_runtime_continuations_with_recovery(&paths, &profiles)
        .expect("legacy continuation backup recovery should succeed");

    assert!(loaded.recovered_from_backup);
    assert_eq!(
        loaded
            .value
            .response_profile_bindings
            .get("resp-legacy")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
}

#[test]
fn runtime_continuations_reject_stale_generation_overwrite() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let initial_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-old".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp() - 10,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuations_for_profiles(&paths, &initial_store, &profiles)
        .expect("initial continuation save should succeed");

    let wrapped_primary: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("initial continuation primary should be readable"),
    )
    .expect("initial continuation primary should be valid json");
    assert_eq!(wrapped_primary["generation"].as_u64(), Some(1));

    let newer_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-new".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    write_versioned_runtime_sidecar(
        &runtime_continuations_file_path(&paths),
        &runtime_continuations_last_good_file_path(&paths),
        2,
        &newer_store,
    );

    let stale_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp() - 20,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    let err = save_runtime_continuations_for_profiles(&paths, &stale_store, &profiles)
        .expect_err("stale continuation save should be fenced");
    assert!(
        err.to_string().contains("stale runtime sidecar generation"),
        "unexpected stale-save error: {err:#}"
    );

    let wrapped_after: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("continuation primary should still be readable"),
    )
    .expect("continuation primary should still be valid json");
    assert_eq!(wrapped_after["generation"].as_u64(), Some(2));
    assert_eq!(
        wrapped_after["value"]["response_profile_bindings"]["resp-new"]["profile_name"],
        serde_json::Value::String("main".to_string())
    );
    assert!(
        !wrapped_after["value"]["response_profile_bindings"]
            .as_object()
            .expect("response_profile_bindings should be an object")
            .contains_key("resp-stale"),
        "stale writer must not overwrite newer continuation state"
    );
}

#[test]
fn runtime_state_snapshot_save_retries_stale_continuation_generation() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let initial_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    initial_state
        .save(&paths)
        .expect("initial state save should succeed");

    let initial_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-initial".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuations_for_profiles(&paths, &initial_store, &profiles)
        .expect("initial continuation save should succeed");

    let external_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-external".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    write_versioned_runtime_sidecar(
        &runtime_continuations_file_path(&paths),
        &runtime_continuations_last_good_file_path(&paths),
        2,
        &external_store,
    );

    let snapshot = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-local".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let revision = AtomicU64::new(1);
    assert!(
        save_runtime_state_snapshot_if_latest(
            &paths,
            &snapshot,
            &runtime_continuation_store_from_app_state(&snapshot),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &RuntimeProfileBackoffs::default(),
            1,
            &revision,
        )
        .expect("state snapshot save should succeed after stale retry")
    );

    let loaded = load_runtime_continuations_with_recovery(&paths, &profiles)
        .expect("continuations should reload")
        .value;
    assert_eq!(
        loaded
            .response_profile_bindings
            .get("resp-local")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    let wrapped: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("continuation primary should be readable"),
    )
    .expect("continuation primary should remain valid json");
    assert_eq!(wrapped["generation"].as_u64(), Some(3));
}

#[test]
fn runtime_state_snapshot_retry_does_not_resurrect_released_response_binding() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let initial_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    initial_state
        .save(&paths)
        .expect("initial state save should succeed");

    let initial_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuations_for_profiles(&paths, &initial_store, &profiles)
        .expect("initial continuation save should succeed");

    let released_store = RuntimeContinuationStore {
        statuses: RuntimeContinuationStatuses {
            response: BTreeMap::from([("resp-stale".to_string(), dead_continuation_status(now))]),
            ..RuntimeContinuationStatuses::default()
        },
        ..RuntimeContinuationStore::default()
    };
    write_versioned_runtime_sidecar(
        &runtime_continuations_file_path(&paths),
        &runtime_continuations_last_good_file_path(&paths),
        2,
        &released_store,
    );

    let snapshot = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let revision = AtomicU64::new(1);
    assert!(
        save_runtime_state_snapshot_if_latest(
            &paths,
            &snapshot,
            &runtime_continuation_store_from_app_state(&snapshot),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &RuntimeProfileBackoffs::default(),
            1,
            &revision,
        )
        .expect("state snapshot save should succeed after stale retry")
    );

    let loaded = load_runtime_continuations_with_recovery(&paths, &profiles)
        .expect("continuations should reload")
        .value;
    assert!(
        !loaded.response_profile_bindings.contains_key("resp-stale"),
        "released response binding must not be resurrected"
    );
    assert_eq!(
        loaded
            .statuses
            .response
            .get("resp-stale")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );

    let state = AppState::load(&paths).expect("state should reload");
    assert!(
        !state.response_profile_bindings.contains_key("resp-stale"),
        "state snapshot must not rewrite the released response binding"
    );
}

#[test]
fn runtime_continuation_journal_save_retries_stale_generation() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
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

    let initial = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-initial".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &initial, now - 30)
        .expect("initial journal save should succeed");

    let external = RuntimeContinuationJournal {
        saved_at: now - 10,
        continuations: RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-external".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now - 10,
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
    };
    write_versioned_runtime_sidecar(
        &runtime_continuation_journal_file_path(&paths),
        &runtime_continuation_journal_last_good_file_path(&paths),
        2,
        &external,
    );

    let incoming = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-local".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &incoming, now)
        .expect("journal save should retry stale generation");

    let loaded = load_runtime_continuation_journal_with_recovery(&paths, &state.profiles)
        .expect("journal should reload")
        .value;
    assert_eq!(loaded.saved_at, now);
    assert_eq!(
        loaded
            .continuations
            .response_profile_bindings
            .get("resp-local")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    let wrapped: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuation_journal_file_path(&paths))
            .expect("journal primary should be readable"),
    )
    .expect("journal primary should remain valid json");
    assert_eq!(wrapped["generation"].as_u64(), Some(3));
}

#[test]
fn runtime_continuation_journal_save_for_profiles_preserves_bindings_without_state_file() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let continuations = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };

    save_runtime_continuation_journal_for_profiles(&paths, &continuations, &profiles, now)
        .expect("journal save with explicit profiles should succeed");

    let loaded = load_runtime_continuation_journal_with_recovery(&paths, &profiles)
        .expect("journal should reload")
        .value;
    assert_eq!(loaded.saved_at, now);
    assert_eq!(
        loaded
            .continuations
            .response_profile_bindings
            .get("resp-main")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
}

#[test]
fn runtime_continuation_journal_retry_does_not_resurrect_released_response_binding() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
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

    let initial = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &initial, now - 30)
        .expect("initial journal save should succeed");

    let external = RuntimeContinuationJournal {
        saved_at: now - 5,
        continuations: RuntimeContinuationStore {
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-stale".to_string(),
                    dead_continuation_status(now),
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
    };
    write_versioned_runtime_sidecar(
        &runtime_continuation_journal_file_path(&paths),
        &runtime_continuation_journal_last_good_file_path(&paths),
        2,
        &external,
    );

    let incoming = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &incoming, now)
        .expect("journal save should retry stale generation");

    let loaded = load_runtime_continuation_journal_with_recovery(&paths, &state.profiles)
        .expect("journal should reload")
        .value;
    assert_eq!(loaded.saved_at, now);
    assert!(
        !loaded
            .continuations
            .response_profile_bindings
            .contains_key("resp-stale"),
        "released response binding must not be resurrected in the journal"
    );
    assert_eq!(
        loaded
            .continuations
            .statuses
            .response
            .get("resp-stale")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}
