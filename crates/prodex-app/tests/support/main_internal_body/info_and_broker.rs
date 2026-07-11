use super::*;

#[path = "info_and_broker_broker_metrics.rs"]
mod broker_metrics;
#[path = "info_and_broker_runtime_load.rs"]
mod runtime_load;

#[test]
fn build_info_quota_aggregate_uses_live_and_snapshot_data() {
    let now = Local::now().timestamp();
    let reports = vec![
        RunProfileProbeReport {
            name: "main".to_string(),
            order_index: 0,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(80, 3_600, 90, 86_400)),
        },
        RunProfileProbeReport {
            name: "second".to_string(),
            order_index: 1,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("timeout".to_string()),
        },
        RunProfileProbeReport {
            name: "api".to_string(),
            order_index: 2,
            auth: AuthSummary {
                label: "api-key".to_string(),
                quota_compatible: false,
            },
            result: Err("api-key auth".to_string()),
        },
    ];
    let snapshots = BTreeMap::from([(
        "second".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: now,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 40,
            five_hour_reset_at: now + 1_800,
            weekly_status: RuntimeQuotaWindowStatus::Thin,
            weekly_remaining_percent: 70,
            weekly_reset_at: now + 7_200,
        },
    )]);

    let aggregate = build_info_quota_aggregate(&reports, &snapshots, now);

    assert_eq!(aggregate.quota_compatible_profiles, 2);
    assert_eq!(aggregate.live_profiles, 1);
    assert_eq!(aggregate.snapshot_profiles, 1);
    assert_eq!(aggregate.unavailable_profiles, 0);
    assert_eq!(aggregate.five_hour_pool_remaining, 120);
    assert_eq!(aggregate.weekly_pool_remaining, 160);
    assert_eq!(aggregate.earliest_five_hour_reset_at, Some(now + 1_800));
    assert_eq!(aggregate.earliest_weekly_reset_at, Some(now + 7_200));
}

#[test]
fn format_info_provider_summary_counts_all_provider_kinds() {
    let profiles = BTreeMap::from([
        (
            "openai".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/openai"),
                managed: true,
                email: None,
                provider: ProfileProvider::Openai,
            },
        ),
        (
            "gemini".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/gemini"),
                managed: true,
                email: None,
                provider: ProfileProvider::Gemini {
                    email: "gemini@example.com".to_string(),
                    project_id: None,
                },
            },
        ),
        (
            "claude".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/claude"),
                managed: true,
                email: None,
                provider: ProfileProvider::Anthropic {
                    account: None,
                    auth_method: None,
                },
            },
        ),
    ]);

    assert_eq!(
        format_info_provider_summary(&profiles),
        "anthropic=1, gemini=1, openai=1"
    );
}

#[test]
fn format_info_provider_capabilities_summary_reports_routes_and_quota_shapes() {
    let profiles = BTreeMap::from([
        (
            "openai".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/openai"),
                managed: true,
                email: None,
                provider: ProfileProvider::Openai,
            },
        ),
        (
            "gemini".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/gemini"),
                managed: true,
                email: None,
                provider: ProfileProvider::Gemini {
                    email: "gemini@example.com".to_string(),
                    project_id: None,
                },
            },
        ),
        (
            "agy".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/agy"),
                managed: true,
                email: None,
                provider: ProfileProvider::Agy { account: None },
            },
        ),
        (
            "kiro".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/kiro"),
                managed: true,
                email: None,
                provider: ProfileProvider::Kiro {
                    auth_key: "kiro:key".to_string(),
                    auth_kind: None,
                    profile_arn: None,
                    profile_name: None,
                    start_url: None,
                    region: None,
                },
            },
        ),
        (
            "copilot".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/copilot"),
                managed: true,
                email: None,
                provider: ProfileProvider::Copilot {
                    host: "github.com".to_string(),
                    login: "octo".to_string(),
                    api_url: "https://api.githubcopilot.com".to_string(),
                    access_type_sku: None,
                    copilot_plan: None,
                },
            },
        ),
    ]);

    assert_eq!(
        format_info_provider_capabilities_summary(&profiles),
        "routes native=1, adapter=3, external-cli=1, unsupported=0; openai-format=4; quota copilot-monthly=1, external-status=2, gemini-buckets=1, openai-windows=1"
    );
}

#[test]
fn secret_backend_summary_rejects_unimplemented_keyring_backend() {
    let _backend = TestEnvVarGuard::set(PRODEX_SECRET_BACKEND_ENV, "keyring");
    let _service = TestEnvVarGuard::set(PRODEX_SECRET_KEYRING_SERVICE_ENV, "prodex");

    let summary = format_secret_backend_summary();
    let json = secret_backend_json_value();

    assert!(summary.contains("invalid"));
    assert!(summary.contains("not implemented"));
    assert_eq!(json["invalid"], serde_json::Value::Bool(true));
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("not implemented")
    );
}

#[test]
fn collect_info_quota_aggregate_ignores_non_codex_runtime_providers() {
    let temp_dir = TestDir::new();
    let gemini_home = temp_dir.path.join("homes/gemini");
    fs::create_dir_all(&gemini_home).expect("create gemini home");
    let state = AppState {
        active_profile: Some("gemini".to_string()),
        profiles: BTreeMap::from([(
            "gemini".to_string(),
            ProfileEntry {
                codex_home: gemini_home,
                managed: true,
                email: Some("gemini@example.com".to_string()),
                provider: ProfileProvider::Gemini {
                    email: "gemini@example.com".to_string(),
                    project_id: None,
                },
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };

    let aggregate = collect_info_quota_aggregate(&paths, &state, Local::now().timestamp());

    assert_eq!(aggregate.quota_compatible_profiles, 0);
    assert_eq!(aggregate.profiles_with_data(), 0);
    assert_eq!(aggregate.unavailable_profiles, 0);
}

#[test]
fn parse_ps_process_rows_and_classify_runtime_prodex_process() {
    let rows = parse_ps_process_rows(
        "  111 prodex /usr/local/bin/prodex run --profile main\n  222 bash bash\n",
    );

    assert_eq!(rows.len(), 2);
    let process = classify_prodex_process_row(rows[0].clone(), 999, Some("prodex"))
        .expect("prodex row should be classified");

    assert_eq!(process.pid, 111);
    assert!(process.runtime);
    assert!(classify_prodex_process_row(rows[1].clone(), 999, Some("prodex")).is_none());
}

#[test]
fn normalize_run_codex_args_keeps_regular_prompt_intact() {
    let args = vec![OsString::from("fix this bug")];
    assert_eq!(normalize_run_codex_args(&args), args);
}

#[test]
fn runtime_proxy_broker_health_endpoint_reports_registered_metadata() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let current_identity = runtime_current_prodex_binary_identity();
    register_runtime_broker_metadata(
        &proxy.log_path,
        RuntimeBrokerMetadata {
            broker_key: runtime_broker_key(&backend.base_url(), false, false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: current_identity.prodex_version.clone(),
            executable_path: current_identity
                .executable_path
                .as_ref()
                .map(|path| path.display().to_string()),
            executable_sha256: current_identity.executable_sha256.clone(),
        },
    );

    let response = Client::builder()
        .build()
        .expect("client")
        .get(format!(
            "http://{}/__prodex/runtime/health",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("runtime broker health request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let health = response
        .json::<RuntimeBrokerHealth>()
        .expect("runtime broker health should decode");
    assert_eq!(health.current_profile, "main");
    assert_eq!(health.instance_token, "instance");
    assert_eq!(health.persistence_role, "owner");
    assert_eq!(
        health.prodex_version.as_deref(),
        Some(runtime_current_prodex_version())
    );
    assert!(health.executable_path.is_some());
    assert!(
        health
            .executable_sha256
            .as_deref()
            .is_some_and(|hash| hash.len() == 64)
    );
}

#[test]
fn runtime_no_proxy_policy_does_not_leak_into_default_proxy_mode() {
    let _runtime_lock = acquire_test_runtime_lock();
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };

    let _proxy = start_runtime_rotation_proxy_with_listen_addr(
        &paths,
        &state,
        "main",
        backend.base_url(),
        false,
        true,
        None,
    )
    .expect("runtime proxy should start");

    assert_eq!(
        runtime_upstream_proxy_mode_label(false),
        "system",
        "no-proxy runtime policy must not become process-global"
    );
    assert_eq!(runtime_upstream_proxy_mode_label(true), "disabled");
}

#[test]
fn runtime_proxy_log_paths_remain_unique_under_parallel_generation() {
    let worker_count = 32;
    let paths_per_worker = 8;
    let barrier = Arc::new(std::sync::Barrier::new(worker_count + 1));
    let (sender, receiver) = mpsc::channel();
    let mut workers = Vec::new();

    for _ in 0..worker_count {
        let barrier = Arc::clone(&barrier);
        let sender = sender.clone();
        workers.push(thread::spawn(move || {
            barrier.wait();
            let paths = (0..paths_per_worker)
                .map(|_| create_runtime_proxy_log_path())
                .collect::<Vec<_>>();
            sender
                .send(paths)
                .expect("parallel log path batch should send");
        }));
    }

    barrier.wait();
    drop(sender);

    let mut all_paths = Vec::new();
    for paths in receiver {
        all_paths.extend(paths);
    }

    for worker in workers {
        worker.join().expect("parallel log path worker should join");
    }

    let unique_paths = all_paths.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        unique_paths.len(),
        all_paths.len(),
        "runtime proxy log paths should stay unique even under parallel creation"
    );
}

#[test]
fn runtime_proxy_broker_activate_endpoint_updates_current_profile() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

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
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    register_runtime_broker_metadata(
        &proxy.log_path,
        RuntimeBrokerMetadata {
            broker_key: runtime_broker_key(&backend.base_url(), false, false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: None,
            executable_sha256: None,
        },
    );

    let client = Client::builder().build().expect("client");
    let activate = client
        .post(format!(
            "http://{}/__prodex/runtime/activate",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .json(&serde_json::json!({
            "current_profile": "second",
        }))
        .send()
        .expect("runtime broker activate request should succeed");
    assert_eq!(activate.status().as_u16(), 200);

    let health = client
        .get(format!(
            "http://{}/__prodex/runtime/health",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("runtime broker health request should succeed")
        .json::<RuntimeBrokerHealth>()
        .expect("runtime broker health should decode");
    assert_eq!(health.current_profile, "second");
}
