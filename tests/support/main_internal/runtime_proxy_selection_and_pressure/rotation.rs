#[test]
fn rotates_profiles_after_current_profile() {
    let state = AppState {
        active_profile: Some("beta".to_string()),
        profiles: BTreeMap::from([
            (
                "alpha".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/alpha"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "beta".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/beta"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "gamma".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/gamma"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    };

    assert_eq!(
        profile_rotation_order(&state, "beta"),
        vec!["gamma".to_string(), "alpha".to_string()]
    );
}

#[test]
fn backend_api_base_url_maps_to_wham_usage() {
    assert_eq!(
        usage_url("https://chatgpt.com/backend-api"),
        "https://chatgpt.com/backend-api/wham/usage"
    );
}

#[test]
fn custom_base_url_maps_to_codex_usage() {
    assert_eq!(
        usage_url("http://127.0.0.1:8080"),
        "http://127.0.0.1:8080/api/codex/usage"
    );
}

#[test]
fn fetch_usage_json_refreshes_access_token_after_401() {
    let temp_dir = TestDir::new();
    let usage_server =
        TokenAwareServer::start_usage("fresh-token", "main-account", "main@example.com");
    let refresh_server = AuthRefreshServer::start("fresh-token", "fresh-refresh-token");
    let _refresh_guard =
        TestEnvVarGuard::set(CODEX_REFRESH_TOKEN_URL_OVERRIDE_ENV, &refresh_server.url());
    let codex_home = temp_dir.path.join("homes/main");
    write_auth_json_with_tokens(
        &codex_home.join("auth.json"),
        "stale-token",
        "main-account",
        Some("stale-refresh-token"),
        None,
    );

    let usage = fetch_usage_json(&codex_home, Some(&usage_server.base_url()))
        .expect("quota fetch should refresh and succeed");

    assert_eq!(usage["email"], "main@example.com");
    assert_eq!(
        usage_server.auth_headers(),
        vec![
            "Bearer stale-token".to_string(),
            "Bearer fresh-token".to_string()
        ]
    );
    assert_eq!(refresh_server.request_bodies().len(), 1);

    let auth_json = fs::read_to_string(codex_home.join("auth.json"))
        .expect("updated auth.json should be readable");
    let auth_json: serde_json::Value =
        serde_json::from_str(&auth_json).expect("updated auth.json should parse");
    assert_eq!(auth_json["tokens"]["access_token"], "fresh-token");
    assert_eq!(auth_json["tokens"]["refresh_token"], "fresh-refresh-token");
    assert!(auth_json.get("last_refresh").is_some());
}

#[test]
fn read_auth_summary_classifies_api_key_auth() {
    let temp_dir = TestDir::new();
    let codex_home = temp_dir.path.join("homes/main");
    write_api_key_auth_json(&codex_home.join("auth.json"));

    let summary = read_auth_summary(&codex_home);
    assert_eq!(summary.label, "api-key");
    assert!(!summary.quota_compatible);
}

#[test]
fn read_auth_summary_classifies_invalid_auth_json() {
    let temp_dir = TestDir::new();
    let codex_home = temp_dir.path.join("homes/main");
    fs::create_dir_all(&codex_home).expect("failed to create codex home");
    fs::write(codex_home.join("auth.json"), "{").expect("failed to write invalid auth.json");

    let summary = read_auth_summary(&codex_home);
    assert_eq!(summary.label, "invalid-auth");
    assert!(!summary.quota_compatible);
}

#[test]
fn read_usage_auth_prefers_account_id_from_access_token_claims() {
    let temp_dir = TestDir::new();
    let codex_home = temp_dir.path.join("homes/main");
    write_auth_json_with_tokens(
        &codex_home.join("auth.json"),
        &fake_jwt_with_exp_and_account_id(Local::now().timestamp() + 600, "jwt-account"),
        "stale-account",
        Some("stale-refresh-token"),
        None,
    );

    let auth = read_usage_auth(&codex_home).expect("usage auth should load");
    assert_eq!(auth.account_id.as_deref(), Some("jwt-account"));
}

#[test]
fn profile_name_is_derived_from_email() {
    assert_eq!(
        profile_name_from_email("Main+Ops@Example.com"),
        "main-ops_example.com"
    );
}

#[test]
fn unique_profile_name_adds_numeric_suffix() {
    let state = AppState {
        active_profile: None,
        profiles: BTreeMap::from([(
            "main_example.com".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/existing"),
                managed: true,
                email: Some("other@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: PathBuf::from("/tmp/prodex-test"),
        state_file: PathBuf::from("/tmp/prodex-test/state.json"),
        managed_profiles_root: PathBuf::from("/tmp/prodex-test/profiles"),
        shared_codex_root: PathBuf::from("/tmp/prodex-test/default-codex"),
        legacy_shared_codex_root: PathBuf::from("/tmp/prodex-test/shared"),
    };

    assert_eq!(
        unique_profile_name_for_email(&paths, &state, "main@example.com"),
        "main_example.com-2"
    );
}

#[test]
fn unique_profile_name_reclaims_untracked_managed_directory() {
    let temp_dir = TestDir::new();
    let state = AppState::default();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let stale_dir = paths.managed_profiles_root.join("main_example.com");
    fs::create_dir_all(&stale_dir).expect("stale managed directory should exist");
    fs::write(stale_dir.join("stale.txt"), "old").expect("stale file should be written");

    assert_eq!(
        unique_profile_name_for_email(&paths, &state, "main@example.com"),
        "main_example.com"
    );
    assert!(
        !stale_dir.exists(),
        "untracked managed directory should be reclaimed before suffixing"
    );
}

#[test]
fn remove_profile_deletes_managed_home_by_default() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let paths = AppPaths::discover().expect("paths should resolve");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profile root should exist");
    let profile_home = paths.managed_profiles_root.join("main");
    fs::create_dir_all(&profile_home).expect("managed profile home should exist");
    fs::write(profile_home.join("auth.json"), "{}").expect("auth file should exist");

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
    write_versioned_runtime_sidecar(
        &runtime_usage_snapshots_file_path(&paths),
        &runtime_usage_snapshots_last_good_file_path(&paths),
        0,
        &BTreeMap::<String, RuntimeProfileUsageSnapshot>::new(),
    );
    write_versioned_runtime_sidecar(
        &runtime_scores_file_path(&paths),
        &runtime_scores_last_good_file_path(&paths),
        0,
        &BTreeMap::<String, RuntimeProfileHealth>::new(),
    );
    write_versioned_runtime_sidecar(
        &runtime_backoffs_file_path(&paths),
        &runtime_backoffs_last_good_file_path(&paths),
        0,
        &RuntimeProfileBackoffs::default(),
    );
    state.save(&paths).expect("state should save");

    handle_remove_profile(RemoveProfileArgs {
        name: Some("main".to_string()),
        all: false,
        delete_home: false,
    })
    .expect("managed profile remove should succeed");

    let reloaded = AppState::load(&paths).expect("state should reload");
    assert!(!reloaded.profiles.contains_key("main"));
    assert!(
        !profile_home.exists(),
        "managed profile home should be deleted even without --delete-home"
    );
}

#[test]
fn remove_all_profiles_clears_state_and_continuation_sidecars() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let paths = AppPaths::discover().expect("paths should resolve");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profile root should exist");
    let managed_home = paths.managed_profiles_root.join("main");
    let external_home = temp_dir.path.join("external-second");
    fs::create_dir_all(&managed_home).expect("managed profile home should exist");
    fs::create_dir_all(&external_home).expect("external profile home should exist");
    fs::write(managed_home.join("auth.json"), "{}").expect("managed auth file should exist");
    fs::write(external_home.join("auth.json"), "{}").expect("external auth file should exist");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: managed_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: external_home.clone(),
                    managed: false,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("main".to_string(), now - 1),
            ("second".to_string(), now),
        ]),
        response_profile_bindings: BTreeMap::from([
            (
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "resp-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        session_profile_bindings: BTreeMap::from([
            (
                "sess-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "sess-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
    };
    write_versioned_runtime_sidecar(
        &runtime_usage_snapshots_file_path(&paths),
        &runtime_usage_snapshots_last_good_file_path(&paths),
        0,
        &BTreeMap::<String, RuntimeProfileUsageSnapshot>::new(),
    );
    write_versioned_runtime_sidecar(
        &runtime_scores_file_path(&paths),
        &runtime_scores_last_good_file_path(&paths),
        0,
        &BTreeMap::<String, RuntimeProfileHealth>::new(),
    );
    write_versioned_runtime_sidecar(
        &runtime_backoffs_file_path(&paths),
        &runtime_backoffs_last_good_file_path(&paths),
        0,
        &RuntimeProfileBackoffs::default(),
    );
    state.save(&paths).expect("state should save");

    let continuations = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([
            (
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "resp-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        session_profile_bindings: BTreeMap::from([
            (
                "sess-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "sess-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        turn_state_bindings: BTreeMap::from([
            (
                "turn-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "turn-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        session_id_bindings: BTreeMap::from([
            (
                "sid-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "sid-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        statuses: RuntimeContinuationStatuses::default(),
    };
    save_runtime_continuations_for_profiles(&paths, &continuations, &state.profiles)
        .expect("continuations should save");
    save_runtime_continuation_journal_for_profiles(&paths, &continuations, &state.profiles, now)
        .expect("continuation journal should save");

    handle_remove_profile(RemoveProfileArgs {
        name: None,
        all: true,
        delete_home: false,
    })
    .expect("bulk profile remove should succeed");

    let reloaded = AppState::load(&paths).expect("state should reload");
    assert!(
        reloaded.profiles.is_empty(),
        "all profiles should be removed"
    );
    assert!(
        reloaded.active_profile.is_none(),
        "active profile should clear"
    );
    assert!(
        reloaded.last_run_selected_at.is_empty(),
        "selection metadata should be cleared"
    );
    assert!(
        reloaded.response_profile_bindings.is_empty(),
        "response bindings should be cleared"
    );
    assert!(
        reloaded.session_profile_bindings.is_empty(),
        "session bindings should be cleared"
    );
    assert!(
        !managed_home.exists(),
        "managed profile home should be deleted during bulk removal"
    );
    assert!(
        external_home.exists(),
        "external profile home should remain without --delete-home"
    );

    let restored_continuations =
        load_runtime_continuations_with_recovery(&paths, &reloaded.profiles)
            .expect("continuations should load")
            .value;
    assert!(
        restored_continuations.response_profile_bindings.is_empty(),
        "response continuation bindings should be cleared"
    );
    assert!(
        restored_continuations.session_profile_bindings.is_empty(),
        "session continuation bindings should be cleared"
    );
    assert!(
        restored_continuations.turn_state_bindings.is_empty(),
        "turn state bindings should be cleared"
    );
    assert!(
        restored_continuations.session_id_bindings.is_empty(),
        "session id bindings should be cleared"
    );

    let restored_journal =
        load_runtime_continuation_journal_with_recovery(&paths, &reloaded.profiles)
            .expect("continuation journal should load")
            .value;
    assert!(
        restored_journal
            .continuations
            .response_profile_bindings
            .is_empty(),
        "journal response bindings should be cleared"
    );
    assert!(
        restored_journal
            .continuations
            .session_profile_bindings
            .is_empty(),
        "journal session bindings should be cleared"
    );
    assert!(
        restored_journal
            .continuations
            .turn_state_bindings
            .is_empty(),
        "journal turn state bindings should be cleared"
    );
    assert!(
        restored_journal
            .continuations
            .session_id_bindings
            .is_empty(),
        "journal session id bindings should be cleared"
    );
}

#[test]
fn remove_all_profiles_rejects_delete_home_for_external_profiles() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let paths = AppPaths::discover().expect("paths should resolve");
    let external_home = temp_dir.path.join("external-second");
    fs::create_dir_all(&external_home).expect("external profile home should exist");
    fs::write(external_home.join("auth.json"), "{}").expect("external auth file should exist");

    AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: external_home.clone(),
                managed: false,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .expect("state should save");

    let err = handle_remove_profile(RemoveProfileArgs {
        name: None,
        all: true,
        delete_home: true,
    })
    .expect_err("bulk delete should reject external homes");
    assert!(
        err.to_string()
            .contains("refuses to delete external profiles"),
        "unexpected error: {err:#}"
    );

    let reloaded = AppState::load(&paths).expect("state should reload");
    assert!(
        reloaded.profiles.contains_key("second"),
        "profile should remain after rejected bulk delete"
    );
    assert!(
        external_home.exists(),
        "external home should remain after rejected bulk delete"
    );
}

#[test]
fn app_paths_discover_uses_prodex_root_for_default_shared_codex_home() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);

    let paths = AppPaths::discover().expect("paths should resolve");
    assert_eq!(paths.root, prodex_home);
    assert_eq!(paths.shared_codex_root, paths.root.join(".codex"));
    assert_eq!(paths.legacy_shared_codex_root, paths.root.join("shared"));
}

#[test]
fn app_paths_discover_resolves_relative_shared_codex_home_inside_prodex_root() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let _shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", ".codex-local");

    let paths = AppPaths::discover().expect("paths should resolve");
    assert_eq!(paths.shared_codex_root, paths.root.join(".codex-local"));
}

#[test]
fn select_default_codex_home_prefers_legacy_home_until_prodex_shared_home_exists() {
    let temp_dir = TestDir::new();
    let shared_codex_home = temp_dir.path.join("prodex/.codex");
    let legacy_codex_home = temp_dir.path.join("home/.codex");
    fs::create_dir_all(&legacy_codex_home).expect("legacy codex home should exist");

    assert_eq!(
        select_default_codex_home(&shared_codex_home, &legacy_codex_home, false),
        legacy_codex_home
    );

    fs::create_dir_all(&shared_codex_home).expect("prodex shared codex home should exist");
    assert_eq!(
        select_default_codex_home(&shared_codex_home, &legacy_codex_home, false),
        shared_codex_home
    );

    fs::remove_dir_all(&shared_codex_home).expect("shared codex home should be removed");
    assert_eq!(
        select_default_codex_home(&shared_codex_home, &legacy_codex_home, true),
        shared_codex_home
    );
}

#[test]
fn parses_email_from_chatgpt_id_token() {
    let id_token = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL3Byb2ZpbGUiOnsiZW1haWwiOiJ1c2VyQGV4YW1wbGUuY29tIn19.c2ln";

    assert_eq!(
        parse_email_from_id_token(id_token).expect("id token should parse"),
        Some("user@example.com".to_string())
    );
}

#[test]
fn usage_response_accepts_null_additional_rate_limits() {
    let usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "email": "user@example.com",
        "plan_type": "plus",
        "rate_limit": null,
        "code_review_rate_limit": null,
        "additional_rate_limits": null
    }))
    .expect("usage response should parse");

    assert!(usage.additional_rate_limits.is_empty());
}

#[test]
fn previous_response_owner_discovery_ignores_retry_backoff() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
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
    };
    let runtime = RuntimeRotationState {
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
        profile_retry_backoff_until: BTreeMap::from([(
            "second".to_string(),
            Local::now().timestamp().saturating_add(60),
        )]),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
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
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let excluded = BTreeSet::from(["main".to_string()]);

    assert_eq!(
        next_runtime_previous_response_candidate(
            &shared,
            &excluded,
            Some("resp-second"),
            RuntimeRouteKind::Responses,
        )
        .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn duplicate_previous_response_owner_verifies_do_not_requeue_persistence() {
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

    let runtime = RuntimeRotationState {
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
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    let initial_second = Local::now().timestamp();
    while Local::now().timestamp() == initial_second {
        thread::sleep(Duration::from_millis(5));
    }

    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("first verification should succeed");
    wait_for_runtime_background_queues_idle();

    let first_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after first bind");
    let binding_marker = "binding previous_response_owner profile=main response_id=resp-1";
    let first_binding_count = first_log.matches(binding_marker).count();
    let first_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(
        first_binding_count, 1,
        "first bind should be persisted once: {first_log}"
    );
    assert_eq!(
        first_revision, 1,
        "first bind should persist once: {first_log}"
    );

    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("duplicate verification should succeed");
    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("second duplicate verification should succeed");
    wait_for_runtime_background_queues_idle();

    let second_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after duplicate binds");
    assert_eq!(
        second_log.matches(binding_marker).count(),
        first_binding_count,
        "duplicate verifies should not re-log identical owner bindings: {second_log}"
    );
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        first_revision,
        "duplicate verifies should not requeue persistence: {second_log}"
    );
}

#[test]
fn previous_response_owner_profile_changes_still_persist() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    create_codex_home_if_missing(&main_home).expect("main profile home should be created");
    create_codex_home_if_missing(&second_home).expect("second profile home should be created");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
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
    };
    state.save(&paths).expect("state should save");

    let runtime = RuntimeRotationState {
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
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-2"),
        RuntimeRouteKind::Websocket,
    )
    .expect("first owner verification should succeed");
    wait_for_runtime_background_queues_idle();
    let initial_log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    let initial_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(
        initial_revision, 1,
        "first owner save should persist once: {initial_log}"
    );

    remember_runtime_successful_previous_response_owner(
        &shared,
        "second",
        Some("resp-2"),
        RuntimeRouteKind::Responses,
    )
    .expect("owner change verification should succeed");
    wait_for_runtime_background_queues_idle();

    let updated_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after rebinding");
    assert!(
        updated_log.contains("binding previous_response_owner profile=second response_id=resp-2"),
        "owner changes should still be logged and persisted: {updated_log}"
    );
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        initial_revision + 1,
        "owner changes should queue a fresh persistence update: {updated_log}"
    );

    let runtime = shared
        .runtime
        .lock()
        .expect("runtime state should remain lockable");
    assert_eq!(
        runtime
            .state
            .response_profile_bindings
            .get("resp-2")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
}

#[test]
fn duplicate_response_ids_do_not_requeue_persistence() {
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

    let runtime = RuntimeRotationState {
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
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    let response_ids = vec!["resp-1".to_string()];
    remember_runtime_response_ids(&shared, "main", &response_ids, RuntimeRouteKind::Responses)
        .expect("first response id bind should succeed");
    wait_for_runtime_background_queues_idle();

    let first_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after first bind");
    let binding_marker = "binding response_ids profile=main count=1 first=Some(\"resp-1\")";
    let first_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(first_log.matches(binding_marker).count(), 1);
    assert_eq!(
        first_revision, 1,
        "first bind should persist once: {first_log}"
    );

    remember_runtime_response_ids(&shared, "main", &response_ids, RuntimeRouteKind::Responses)
        .expect("duplicate response id bind should succeed");
    remember_runtime_response_ids(&shared, "main", &response_ids, RuntimeRouteKind::Responses)
        .expect("second duplicate response id bind should succeed");
    wait_for_runtime_background_queues_idle();

    let second_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after duplicate binds");
    assert_eq!(
        second_log.matches(binding_marker).count(),
        1,
        "duplicate response id binds should not re-log: {second_log}"
    );
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        first_revision,
        "duplicate response id binds should not requeue persistence: {second_log}"
    );
}

#[test]
fn duplicate_non_response_continuation_verifies_do_not_requeue_persistence() {
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

    let runtime = RuntimeRotationState {
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
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    remember_runtime_turn_state(&shared, "main", Some("turn-1"), RuntimeRouteKind::Responses)
        .expect("turn state verification should succeed");
    remember_runtime_session_id(
        &shared,
        "main",
        Some("session-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("session id verification should succeed");
    remember_runtime_compact_lineage(
        &shared,
        "main",
        Some("session-compact"),
        Some("turn-compact"),
        RuntimeRouteKind::Compact,
    )
    .expect("compact lineage verification should succeed");
    wait_for_runtime_background_queues_idle();

    let first_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after initial bindings");
    let first_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(
        first_revision, 3,
        "initial binds should persist once each: {first_log}"
    );
    assert_eq!(
        first_log
            .matches("binding turn_state profile=main value=turn-1")
            .count(),
        1,
        "turn state should be logged once: {first_log}"
    );
    assert_eq!(
        first_log
            .matches("binding session_id profile=main value=session-1")
            .count(),
        1,
        "session id should be logged once: {first_log}"
    );

    remember_runtime_turn_state(&shared, "main", Some("turn-1"), RuntimeRouteKind::Responses)
        .expect("duplicate turn state verification should succeed");
    remember_runtime_session_id(
        &shared,
        "main",
        Some("session-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("duplicate session id verification should succeed");
    remember_runtime_compact_lineage(
        &shared,
        "main",
        Some("session-compact"),
        Some("turn-compact"),
        RuntimeRouteKind::Compact,
    )
    .expect("duplicate compact lineage verification should succeed");
    wait_for_runtime_background_queues_idle();

    let second_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after duplicate bindings");
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        first_revision,
        "duplicate verifies should not requeue persistence: {second_log}"
    );
    assert_eq!(
        second_log
            .matches("binding turn_state profile=main value=turn-1")
            .count(),
        1,
        "duplicate turn state verifies should not re-log: {second_log}"
    );
    assert_eq!(
        second_log
            .matches("binding session_id profile=main value=session-1")
            .count(),
        1,
        "duplicate session id verifies should not re-log: {second_log}"
    );
}

#[test]
fn runtime_affinity_touch_lookups_do_not_requeue_persistence_before_interval() {
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
    let now = Local::now().timestamp();
    let compact_session_key = runtime_compact_session_lineage_key("session-compact");
    let compact_turn_state_key = runtime_compact_turn_state_lineage_key("turn-compact");
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
            "resp-1".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "session-1".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
    };
    state.save(&paths).expect("state should save");

    let verified_status = |route: &str| RuntimeContinuationBindingStatus {
        state: RuntimeContinuationBindingLifecycle::Verified,
        confidence: 1,
        last_touched_at: Some(now),
        last_verified_at: Some(now),
        last_verified_route: Some(route.to_string()),
        last_not_found_at: None,
        not_found_streak: 0,
        success_count: 1,
        failure_count: 0,
    };

    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::from([
            (
                "turn-1".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                compact_turn_state_key.clone(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        session_id_bindings: BTreeMap::from([
            (
                "session-1".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                compact_session_key.clone(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        continuation_statuses: RuntimeContinuationStatuses {
            response: BTreeMap::from([("resp-1".to_string(), verified_status("responses"))]),
            turn_state: BTreeMap::from([
                ("turn-1".to_string(), verified_status("responses")),
                (compact_turn_state_key.clone(), verified_status("compact")),
            ]),
            session_id: BTreeMap::from([
                ("session-1".to_string(), verified_status("websocket")),
                (compact_session_key.clone(), verified_status("compact")),
            ]),
        },
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    for _ in 0..3 {
        assert_eq!(
            runtime_response_bound_profile(&shared, "resp-1", RuntimeRouteKind::Responses)
                .expect("response owner lookup should succeed"),
            Some("main".to_string())
        );
        assert_eq!(
            runtime_turn_state_bound_profile(&shared, "turn-1")
                .expect("turn state lookup should succeed"),
            Some("main".to_string())
        );
        assert_eq!(
            runtime_session_bound_profile(&shared, "session-1")
                .expect("session lookup should succeed"),
            Some("main".to_string())
        );
        assert_eq!(
            runtime_compact_route_followup_bound_profile(&shared, Some("turn-compact"), None)
                .expect("compact turn-state lookup should succeed"),
            Some(("main".to_string(), "turn_state"))
        );
        assert_eq!(
            runtime_compact_route_followup_bound_profile(&shared, None, Some("session-compact"))
                .expect("compact session lookup should succeed"),
            Some(("main".to_string(), "session_id"))
        );
    }
    wait_for_runtime_background_queues_idle();

    let log = fs::read_to_string(&shared.log_path).unwrap_or_default();
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        0,
        "fresh touch lookups should not requeue persistence before the interval elapses: {log}"
    );
    assert!(
        !log.contains("response_touch:resp-1")
            && !log.contains("turn_state_touch:turn-1")
            && !log.contains("session_touch:session-1")
            && !log.contains("compact_turn_state_touch:turn-compact")
            && !log.contains("compact_session_touch:session-compact"),
        "touch lookups should stay in-memory until the persistence interval elapses: {log}"
    );
}

#[test]
fn previous_response_release_preserves_session_and_compact_session_lineage_for_compact_followups() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
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
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: backend.base_url(),
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

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "second",
        &[String::from("resp-second")],
        Some("turn-second"),
        RuntimeRouteKind::Responses,
    )
    .expect("response affinity should be recorded");
    remember_runtime_turn_state(
        &shared,
        "second",
        Some("turn-second"),
        RuntimeRouteKind::Responses,
    )
    .expect("turn-state affinity should be recorded");
    remember_runtime_session_id(
        &shared,
        "second",
        Some("sess-compact"),
        RuntimeRouteKind::Websocket,
    )
    .expect("session affinity should be recorded");
    remember_runtime_compact_lineage(
        &shared,
        "second",
        Some("sess-compact"),
        None,
        RuntimeRouteKind::Compact,
    )
    .expect("compact session lineage should be recorded");
    wait_for_runtime_background_queues_idle();

    assert!(!release_runtime_previous_response_affinity(
        &shared,
        "second",
        Some("resp-second"),
        Some("turn-second"),
        Some("sess-compact"),
        RuntimeRouteKind::Responses,
    )
    .expect("first not-found should defer release"));
    assert!(release_runtime_previous_response_affinity(
        &shared,
        "second",
        Some("resp-second"),
        Some("turn-second"),
        Some("sess-compact"),
        RuntimeRouteKind::Responses,
    )
    .expect("second not-found should release response and turn-state affinity"));
    wait_for_runtime_background_queues_idle();

    let compact_session_key = runtime_compact_session_lineage_key("sess-compact");
    {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            !runtime
                .state
                .response_profile_bindings
                .contains_key("resp-second"),
            "released previous_response affinity should be removed"
        );
        assert!(
            !runtime.turn_state_bindings.contains_key("turn-second"),
            "released turn_state affinity should be removed"
        );
        assert!(
            runtime.session_id_bindings.contains_key("sess-compact"),
            "existing session affinity should survive response/turn-state release"
        );
        assert!(
            runtime
                .state
                .session_profile_bindings
                .contains_key("sess-compact"),
            "persisted session affinity should survive response/turn-state release"
        );
        assert!(
            runtime
                .session_id_bindings
                .contains_key(&compact_session_key),
            "compact session lineage should survive response/turn-state release"
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .response
                .get("resp-second")
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Dead)
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .turn_state
                .get("turn-second")
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Dead)
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .session_id
                .get("sess-compact")
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Verified)
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .session_id
                .get(&compact_session_key)
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Verified)
        );
    }

    assert_eq!(
        runtime_compact_route_followup_bound_profile(&shared, None, Some("sess-compact"))
            .expect("compact session lookup should succeed"),
        Some(("second".to_string(), "session_id"))
    );

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-compact".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };
    let _response = proxy_runtime_standard_request(1, &request, &shared)
        .expect("compact follow-up request should succeed");
    wait_for_runtime_background_queues_idle();

    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
    let log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after compact follow-up");
    assert!(
        log.contains("compact_followup_owner profile=second source=session_id"),
        "compact follow-up should still resolve the preserved session lineage owner: {log}"
    );
}

#[test]
fn clear_runtime_stale_previous_response_binding_marks_dead_tombstone() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
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
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
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
        },
        usize::MAX,
    );

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "main",
        &[String::from("resp-main")],
        Some("turn-main"),
        RuntimeRouteKind::Responses,
    )
    .expect("response affinity should be recorded");
    wait_for_runtime_background_queues_idle();

    assert!(
        clear_runtime_stale_previous_response_binding(&shared, "main", Some("resp-main"))
            .expect("stale clear should succeed")
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key("resp-main"),
        "cleared stale binding should be removed from live affinity"
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn websocket_success_without_turn_state_keeps_compact_lineage_alive() {
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([(
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "second".to_string(),
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
        },
        usize::MAX,
    );

    remember_runtime_session_id(
        &shared,
        "second",
        Some("sess-compact"),
        RuntimeRouteKind::Websocket,
    )
    .expect("session id affinity should be recorded");
    remember_runtime_compact_lineage(
        &shared,
        "second",
        Some("sess-compact"),
        None,
        RuntimeRouteKind::Compact,
    )
    .expect("compact lineage should be recorded");
    wait_for_runtime_background_queues_idle();

    let mut previous_response_owner_recorded = false;
    remember_runtime_websocket_response_ids(
        RuntimeWebsocketResponseBindingContext {
            shared: &shared,
            profile_name: "second",
            request_previous_response_id: None,
            request_session_id: Some("sess-compact"),
            request_turn_state: None,
            response_turn_state: None,
        },
        &[String::from("resp-after-compact")],
        &mut previous_response_owner_recorded,
    )
    .expect("websocket response ids should be recorded");
    wait_for_runtime_background_queues_idle();

    let compact_session_key = runtime_compact_session_lineage_key("sess-compact");
    {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            runtime.session_id_bindings.contains_key(&compact_session_key),
            "compact session lineage should survive until a successor turn_state exists"
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .session_id
                .get(&compact_session_key)
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Verified)
        );
    }

    assert_eq!(
        runtime_compact_route_followup_bound_profile(&shared, None, Some("sess-compact"))
            .expect("compact session lookup should succeed"),
        Some(("second".to_string(), "session_id"))
    );

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("turn_state_coverage route=websocket profile=second status=missing"),
        "missing websocket turn_state should be logged: {log}"
    );
    assert!(
        !log.contains("compact_lineage_released profile=second reason=response_committed"),
        "compact lineage should not be released before a successor turn_state exists: {log}"
    );
}

#[test]
fn http_responses_success_without_turn_state_keeps_compact_lineage_alive() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([(
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "second".to_string(),
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
        },
        usize::MAX,
    );

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "second",
        &[String::from("resp-second")],
        Some("turn-second"),
        RuntimeRouteKind::Responses,
    )
    .expect("response affinity should be recorded");
    remember_runtime_session_id(
        &shared,
        "second",
        Some("sess-compact"),
        RuntimeRouteKind::Websocket,
    )
    .expect("session affinity should be recorded");
    remember_runtime_compact_lineage(
        &shared,
        "second",
        Some("sess-compact"),
        None,
        RuntimeRouteKind::Compact,
    )
    .expect("compact lineage should be recorded");
    wait_for_runtime_background_queues_idle();

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-compact".to_string()),
        ],
        body: br#"{"input":[],"stream":true,"previous_response_id":"resp-second"}"#.to_vec(),
    };
    let inflight_guard = acquire_runtime_profile_inflight_guard(&shared, "second", "responses_http")
        .expect("responses inflight guard should be acquired");
    let response = send_runtime_proxy_upstream_responses_request(1, &request, &shared, "second", None)
        .expect("responses follow-up should reach upstream");
    match prepare_runtime_proxy_responses_success(
        RuntimeResponsesSuccessContext {
            request_id: 1,
            request_previous_response_id: Some("resp-second"),
            request_session_id: Some("sess-compact"),
            request_turn_state: None,
            turn_state_override: None,
            shared: &shared,
            profile_name: "second",
            inflight_guard,
        },
        response,
    )
    .expect("responses follow-up should prepare successfully")
    {
        RuntimeResponsesAttempt::Success { .. } => {}
        _ => panic!("unexpected non-success responses attempt"),
    }
    wait_for_runtime_background_queues_idle();

    let compact_session_key = runtime_compact_session_lineage_key("sess-compact");
    {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            runtime.session_id_bindings.contains_key(&compact_session_key),
            "compact session lineage should survive until a successor turn_state exists"
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .session_id
                .get(&compact_session_key)
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Verified)
        );
    }

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("turn_state_coverage route=responses profile=second status=missing"),
        "missing responses turn_state should be logged: {log}"
    );
    assert!(
        !log.contains("compact_lineage_released profile=second reason=response_committed"),
        "compact lineage should not be released before a successor turn_state exists: {log}"
    );
}

#[test]
fn websocket_success_with_turn_state_releases_compact_lineage() {
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([(
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "second".to_string(),
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
        },
        usize::MAX,
    );

    remember_runtime_session_id(
        &shared,
        "second",
        Some("sess-compact"),
        RuntimeRouteKind::Websocket,
    )
    .expect("session id affinity should be recorded");
    remember_runtime_compact_lineage(
        &shared,
        "second",
        Some("sess-compact"),
        None,
        RuntimeRouteKind::Compact,
    )
    .expect("compact lineage should be recorded");
    wait_for_runtime_background_queues_idle();

    let mut previous_response_owner_recorded = false;
    remember_runtime_websocket_response_ids(
        RuntimeWebsocketResponseBindingContext {
            shared: &shared,
            profile_name: "second",
            request_previous_response_id: None,
            request_session_id: Some("sess-compact"),
            request_turn_state: None,
            response_turn_state: Some("turn-successor"),
        },
        &[String::from("resp-after-compact")],
        &mut previous_response_owner_recorded,
    )
    .expect("websocket response ids should be recorded");
    wait_for_runtime_background_queues_idle();

    let compact_session_key = runtime_compact_session_lineage_key("sess-compact");
    {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            !runtime.session_id_bindings.contains_key(&compact_session_key),
            "compact session lineage should be released once a successor turn_state exists"
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .session_id
                .get(&compact_session_key)
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Dead)
        );
    }

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("compact_lineage_released profile=second reason=response_committed"),
        "compact lineage release should be logged after successor turn_state is established: {log}"
    );
}

#[test]
fn http_responses_success_with_turn_state_releases_compact_lineage() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_sse_headers_array_turn_state();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([(
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "second".to_string(),
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
        },
        usize::MAX,
    );

    remember_runtime_session_id(
        &shared,
        "second",
        Some("sess-compact"),
        RuntimeRouteKind::Websocket,
    )
    .expect("session affinity should be recorded");
    remember_runtime_compact_lineage(
        &shared,
        "second",
        Some("sess-compact"),
        None,
        RuntimeRouteKind::Compact,
    )
    .expect("compact lineage should be recorded");
    wait_for_runtime_background_queues_idle();

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-compact".to_string()),
        ],
        body: br#"{"input":[],"stream":true}"#.to_vec(),
    };
    let inflight_guard = acquire_runtime_profile_inflight_guard(&shared, "second", "responses_http")
        .expect("responses inflight guard should be acquired");
    let response = send_runtime_proxy_upstream_responses_request(1, &request, &shared, "second", None)
        .expect("responses request should reach upstream");
    match prepare_runtime_proxy_responses_success(
        RuntimeResponsesSuccessContext {
            request_id: 1,
            request_previous_response_id: None,
            request_session_id: Some("sess-compact"),
            request_turn_state: None,
            turn_state_override: None,
            shared: &shared,
            profile_name: "second",
            inflight_guard,
        },
        response,
    )
    .expect("responses request should prepare successfully")
    {
        RuntimeResponsesAttempt::Success { .. } => {}
        _ => panic!("unexpected non-success responses attempt"),
    }
    wait_for_runtime_background_queues_idle();

    let compact_session_key = runtime_compact_session_lineage_key("sess-compact");
    {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            runtime.turn_state_bindings.contains_key("turn-second"),
            "successor response turn_state should be remembered before compact lineage release"
        );
        assert!(
            !runtime.session_id_bindings.contains_key(&compact_session_key),
            "compact session lineage should be released once a successor turn_state exists"
        );
        assert_eq!(
            runtime
                .continuation_statuses
                .session_id
                .get(&compact_session_key)
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Dead)
        );
    }

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("compact_lineage_released profile=second reason=response_committed"),
        "compact lineage release should be logged after successor turn_state is established: {log}"
    );
}

#[test]
fn runtime_rotation_proxy_can_start_even_if_selected_profile_auth_is_not_quota_compatible() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    create_codex_home_if_missing(&main_home).expect("main home should be created");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
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
    };

    assert!(should_enable_runtime_rotation_proxy(&state, "main", true));
}

#[test]
fn runtime_rotation_proxy_stays_disabled_without_any_quota_compatible_profile() {
    let temp_dir = TestDir::new();
    let state = AppState {
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
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    assert!(!should_enable_runtime_rotation_proxy(&state, "main", true));
}

#[test]
fn optimistic_current_candidate_skips_transport_backoff() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
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
        profile_health: BTreeMap::new(),
    };
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
        runtime: Arc::new(Mutex::new(runtime)),
    };

    mark_runtime_profile_transport_backoff(&shared, "main", RuntimeRouteKind::Responses, "test")
        .expect("transport backoff should be recorded");

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn precommit_budget_exhausts_by_attempt_limit_or_elapsed_time() {
    assert!(!runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        0,
        false,
        false
    ));
    assert!(runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
        false,
        false
    ));

    let started_at = Instant::now()
        .checked_sub(Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_BUDGET_MS + 1))
        .expect("elapsed start should be constructible");
    assert!(runtime_proxy_precommit_budget_exhausted(
        started_at, 0, false, false
    ));
    assert!(!runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
        true,
        false
    ));
}

#[test]
fn websocket_previous_response_not_found_requires_stale_continuation_without_turn_state() {
    assert!(
        runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            true,
            Some(RuntimePreviousResponseFreshFallbackShape::ContinuationOnly),
        ),
        "locked-affinity websocket continuations without replayable turn state must fail locally"
    );
    assert!(
        !runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            true,
            true,
            Some(RuntimePreviousResponseFreshFallbackShape::ContinuationOnly),
        ),
        "available replay turn state should keep previous_response retries alive"
    );
    assert!(
        !runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::ReplayableInput),
        ),
        "fresh fallback remains available when the websocket request can be replayed safely"
    );
    assert!(
        runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::ContinuationOnly),
        ),
        "continuation-only websocket requests without turn state must fail locally instead of surfacing raw upstream 400s"
    );
    assert!(
        runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            false,
            None,
        ),
        "unknown fallback shape should stay conservative when websocket turn state is missing"
    );
    assert!(
        !runtime_websocket_previous_response_not_found_requires_stale_continuation(
            None,
            false,
            true,
            Some(RuntimePreviousResponseFreshFallbackShape::ContinuationOnly),
        ),
        "requests without previous_response_id should not be classified as stale continuations"
    );
}

#[test]
fn websocket_reuse_watchdog_fresh_fallback_stays_blocked_for_locked_affinity() {
    assert!(
        !runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed(
            RuntimeWebsocketReuseWatchdogPreviousResponseFallback {
                profile_name: "second",
                previous_response_id: Some("resp-second"),
                previous_response_fresh_fallback_used: false,
                bound_profile: Some("second"),
                pinned_profile: None,
                request_requires_previous_response_affinity: true,
                trusted_previous_response_affinity: false,
                request_turn_state: None,
            },
        ),
        "watchdog fallback should stay blocked for non-replayable locked-affinity continuations"
    );
}

#[test]
fn noncompact_session_priority_ignores_compact_session_profile() {
    assert_eq!(
        runtime_noncompact_session_priority_profile(Some("main"), Some("main")),
        None
    );
    assert_eq!(
        runtime_noncompact_session_priority_profile(Some("second"), Some("main")),
        Some("second")
    );
    assert_eq!(
        runtime_noncompact_session_priority_profile(Some("main"), None),
        Some("main")
    );
}

#[test]
fn optimistic_current_candidate_requires_quota_evidence_when_alternatives_exist() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
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
    };
    let runtime = RuntimeRotationState {
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
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
        )
        .expect("responses candidate lookup should succeed"),
        None
    );
    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
        )
        .expect("websocket candidate lookup should succeed"),
        None
    );
    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("standard candidate lookup should succeed"),
        None
    );
    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Compact,
        )
        .expect("compact candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_recently_unhealthy_profile() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
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
        profile_health: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                updated_at: Local::now().timestamp(),
            },
        )]),
    };
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
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_busy_profile() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
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
        profile_inflight: BTreeMap::from([(
            "main".to_string(),
            RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT,
        )]),
        profile_health: BTreeMap::new(),
    };
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
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_thin_long_lived_quota() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
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
                result: Ok(usage_with_main_windows(9, 18_000, 18, 604_800)),
            },
        )]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
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
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_cached_usage_exhausted_profile() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileProbeCacheEntry {
                checked_at: Local::now().timestamp(),
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(0, 18_000, 50, 604_800)),
            },
        )]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
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
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn direct_current_fallback_profile_bypasses_local_selection_penalties() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
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
        profile_retry_backoff_until: BTreeMap::from([("main".to_string(), now + 60)]),
        profile_transport_backoff_until: BTreeMap::from([(
            runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Standard),
            now + 60,
        )]),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::from([(
            "main".to_string(),
            RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT,
        )]),
        profile_health: BTreeMap::from([(
            runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                updated_at: now,
            },
        )]),
    };
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
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_direct_current_fallback_profile(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("direct fallback lookup should succeed"),
        Some("main".to_string())
    );
}

#[test]
fn direct_current_fallback_profile_is_route_aware_for_heavy_routes() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
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
        profile_inflight: BTreeMap::from([(
            "main".to_string(),
            RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT.saturating_sub(1),
        )]),
        profile_health: BTreeMap::new(),
    };
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
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_direct_current_fallback_profile(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("standard direct fallback lookup should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        runtime_proxy_direct_current_fallback_profile(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
        )
        .expect("responses direct fallback lookup should succeed"),
        None
    );
}
