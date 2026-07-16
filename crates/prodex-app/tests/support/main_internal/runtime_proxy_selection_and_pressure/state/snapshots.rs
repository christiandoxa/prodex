use super::*;

#[test]
fn runtime_state_snapshot_save_preserves_concurrent_profiles() {
    let temp_dir = TestDir::isolated();
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
        save_runtime_state_snapshot_if_latest(RuntimeStateSnapshotSaveInput {
            paths: &paths,
            snapshot: &snapshot,
            continuations: &runtime_continuation_store_from_app_state(&snapshot),
            profile_scores: &BTreeMap::new(),
            usage_snapshots: &BTreeMap::new(),
            backoffs: &RuntimeProfileBackoffs::default(),
            revision: 1,
            latest_revision: &revision,
        })
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
    let temp_dir = TestDir::isolated();
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
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        auto_redeem_enabled: false,
        upstream_no_proxy: false,
        compact_client: reqwest::Client::new(),
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
    schedule_runtime_state_save_request(
        &shared,
        RuntimeStateSaveRequest::from_snapshot(
            RuntimeStateSaveSnapshot {
                paths: paths.clone(),
                state: first_state.clone(),
                continuations: runtime_continuation_store_from_app_state(&first_state),
                profile_scores: BTreeMap::from([(
                    runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
                    RuntimeProfileHealth {
                        score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                        updated_at: now - 20,
                    },
                )]),
                usage_snapshots: BTreeMap::new(),
                backoffs: RuntimeProfileBackoffs {
                    retry_backoff_until: BTreeMap::from([("main".to_string(), now + 60)]),
                    transport_backoff_until: BTreeMap::new(),
                    route_circuit_open_until: BTreeMap::new(),
                },
            },
            RuntimeStateMutation::FullState,
        ),
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
    schedule_runtime_state_save_request(
        &shared,
        RuntimeStateSaveRequest::from_snapshot(
            RuntimeStateSaveSnapshot {
                paths: paths.clone(),
                state: second_state.clone(),
                continuations: runtime_continuation_store_from_app_state(&second_state),
                profile_scores: BTreeMap::from([(
                    runtime_profile_route_health_key("second", RuntimeRouteKind::Compact),
                    RuntimeProfileHealth {
                        score: RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                        updated_at: now - 10,
                    },
                )]),
                usage_snapshots: BTreeMap::new(),
                backoffs: RuntimeProfileBackoffs {
                    retry_backoff_until: BTreeMap::new(),
                    transport_backoff_until: BTreeMap::from([(
                        runtime_profile_transport_backoff_key(
                            "second",
                            RuntimeRouteKind::Responses,
                        ),
                        now + 120,
                    )]),
                    route_circuit_open_until: BTreeMap::new(),
                },
            },
            RuntimeStateMutation::FullState,
        ),
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
fn runtime_state_save_sections_follow_typed_mutation_scope() {
    assert_eq!(
        runtime_state_save_sections(&RuntimeStateMutation::UsageSnapshot("main".into())),
        RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::None,
            continuations: false,
            profile_scores: false,
            usage_snapshots: true,
            backoffs: true,
        }
    );
    assert_eq!(
        runtime_state_save_sections(&RuntimeStateMutation::ResponseIds("main".into())),
        RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::Core,
            continuations: true,
            profile_scores: true,
            usage_snapshots: false,
            backoffs: false,
        }
    );
    assert_eq!(
        runtime_state_save_sections(&RuntimeStateMutation::ProfileCommit("second".into())),
        RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::Core,
            continuations: false,
            profile_scores: true,
            usage_snapshots: false,
            backoffs: true,
        }
    );
    assert_eq!(
        runtime_state_save_sections(&RuntimeStateMutation::StartupAudit),
        RuntimeStateSaveSections::full()
    );
}

#[test]
fn runtime_state_selected_snapshot_preserves_unselected_sections() {
    let temp_dir = TestDir::isolated();
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
    let temp_dir = TestDir::isolated();
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
    let temp_dir = TestDir::isolated();
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

    let err = save_runtime_state_snapshot_if_latest(RuntimeStateSnapshotSaveInput {
        paths: &paths,
        snapshot: &snapshot,
        continuations: &runtime_continuation_store_from_app_state(&snapshot),
        profile_scores: &BTreeMap::new(),
        usage_snapshots: &BTreeMap::new(),
        backoffs: &RuntimeProfileBackoffs::default(),
        revision: 1,
        latest_revision: &latest_revision,
    })
    .expect_err("injected save failure should bubble up");
    assert!(
        err.to_string()
            .contains("injected runtime state save failure")
    );
}

#[test]
fn app_state_load_uses_last_good_backup_when_primary_is_invalid() {
    let temp_dir = TestDir::isolated();
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
