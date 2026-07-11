use super::*;

#[test]
fn runtime_doctor_collect_state_flags_runtime_broker_binary_mismatch() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");

    let server = TinyServer::http("127.0.0.1:0").expect("health server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("health server should expose a TCP address");
    save_runtime_broker_registry(
        &paths,
        "doctor-mismatch",
        &RuntimeBrokerRegistry {
            pid: std::process::id(),
            listen_addr: listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            smart_context_enabled: false,
            current_profile: "main".to_string(),
            instance_id: "instance".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        },
    )
    .expect("doctor mismatch registry should save");
    save_runtime_broker_test_capability(&paths, "doctor-mismatch", "instance", "secret");

    let health_thread = thread::spawn(move || {
        let request = server.recv().expect("health request should arrive");
        let body = serde_json::to_string(&RuntimeBrokerHealth {
            pid: std::process::id(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            active_requests: 0,
            instance_id: "instance".to_string(),
            persistence_role: "owner".to_string(),
            prodex_version: Some("0.26.0".to_string()),
            executable_path: Some("/tmp/prodex-0.26.0".to_string()),
            executable_sha256: None,
        })
        .expect("health payload should serialize");
        let response = TinyResponse::from_string(body).with_status_code(200);
        request
            .respond(response)
            .expect("health response should write");
    });

    let mut summary = RuntimeDoctorSummary::default();
    collect_runtime_doctor_state(&paths, &mut summary);

    health_thread
        .join()
        .expect("health server thread should join");

    assert!(
        summary.runtime_broker_mismatch,
        "doctor should flag mismatched live runtime broker identity"
    );
    assert!(
        summary.runtime_broker_identities.iter().any(|line| line
            .contains("broker_key=doctor-mismatch")
            && line.contains("status=binary_mismatch")
            && line.contains("mismatch=version_mismatch")
            && line.contains("version=0.26.0")
            && line.contains("source=health")),
        "doctor should surface the mismatched broker identity: {:?}",
        summary.runtime_broker_identities
    );
    summary.pointer_exists = true;
    summary.log_exists = true;
    summary.line_count = 1;
    runtime_doctor_finalize_summary(&mut summary);
    assert!(
        summary
            .diagnosis
            .contains("Runtime broker doctor-mismatch pid"),
        "doctor should explain how to resolve live broker mismatch: {}",
        summary.diagnosis
    );
}

#[test]
fn runtime_doctor_collect_state_surfaces_dead_broker_registry_and_stale_leases() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");

    save_runtime_broker_registry(
        &paths,
        "doctor-dead",
        &RuntimeBrokerRegistry {
            pid: 999_999,
            listen_addr: "127.0.0.1:9".to_string(),
            started_at: Local::now().timestamp(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            smart_context_enabled: false,
            current_profile: "main".to_string(),
            instance_id: "dead-instance".to_string(),
            prodex_version: Some("0.1.0".to_string()),
            executable_path: Some("/tmp/old-prodex".to_string()),
            executable_sha256: Some("deadbeef".to_string()),
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        },
    )
    .expect("dead broker registry should save");
    let lease_dir = runtime_broker_lease_dir(&paths, "doctor-dead");
    fs::create_dir_all(&lease_dir).expect("lease dir should exist");
    fs::write(lease_dir.join("stale.lease"), "pid=999998\n").expect("stale lease should write");

    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    collect_runtime_doctor_state(&paths, &mut summary);
    runtime_doctor_finalize_summary(&mut summary);

    assert!(
        summary
            .runtime_broker_identities
            .iter()
            .any(|line| line.contains("broker_key=doctor-dead")
                && line.contains("status=dead_pid")
                && line.contains("stale_leases=1")),
        "doctor should keep dead registry artifacts visible: {:?}",
        summary.runtime_broker_identities
    );
    assert!(
        summary
            .diagnosis
            .contains("run `prodex cleanup` or restart `prodex run`"),
        "doctor should point to cleanup/restart action for dead broker registry: {}",
        summary.diagnosis
    );
}

#[test]
fn runtime_doctor_collect_state_surfaces_unreachable_live_broker_health() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let _connect_timeout_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS", "20");
    let _read_timeout_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS", "20");
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");

    let server = TinyServer::http("127.0.0.1:0").expect("health timeout server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("health timeout server should expose a TCP address");
    save_runtime_broker_registry(
        &paths,
        "doctor-timeout",
        &RuntimeBrokerRegistry {
            pid: std::process::id(),
            listen_addr: listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            smart_context_enabled: false,
            current_profile: "main".to_string(),
            instance_id: "timeout-instance".to_string(),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: env::current_exe()
                .ok()
                .map(|path| path.display().to_string()),
            executable_sha256: None,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        },
    )
    .expect("timeout broker registry should save");
    save_runtime_broker_test_capability(&paths, "doctor-timeout", "timeout-instance", "secret");

    let health_thread = thread::spawn(move || {
        let request = server.recv().expect("timeout health request should arrive");
        thread::sleep(Duration::from_millis(80));
        let _ = request.respond(TinyResponse::from_string("{}").with_status_code(200));
    });

    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    collect_runtime_doctor_state(&paths, &mut summary);
    runtime_doctor_finalize_summary(&mut summary);
    health_thread
        .join()
        .expect("timeout health thread should join");

    assert!(
        summary
            .runtime_broker_identities
            .iter()
            .any(|line| line.contains("broker_key=doctor-timeout")
                && line.contains("status=health_timeout")),
        "doctor should classify timed-out health probes: {:?}",
        summary.runtime_broker_identities
    );
    assert!(
        summary.diagnosis.contains("health probe timed out"),
        "doctor should surface timeout-specific action text: {}",
        summary.diagnosis
    );
}

#[test]
fn collect_orphan_managed_profile_dirs_ignores_tracked_and_fresh_dirs() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");

    let tracked = paths.managed_profiles_root.join("tracked");
    fs::create_dir_all(&tracked).expect("tracked dir should exist");
    fs::write(tracked.join("auth.json"), "{}").expect("tracked auth should be written");

    let orphan = paths.managed_profiles_root.join("orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan auth should be written");

    let fresh = paths.managed_profiles_root.join("fresh");
    fs::create_dir_all(&fresh).expect("fresh dir should exist");
    fs::write(fresh.join("auth.json"), "{}").expect("fresh auth should be written");

    let state = AppState {
        active_profile: Some("tracked".to_string()),
        profiles: BTreeMap::from([(
            "tracked".to_string(),
            ProfileEntry {
                codex_home: tracked,
                managed: true,
                email: Some("tracked@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    let future_now = SystemTime::now()
        + Duration::from_secs(ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS as u64 + 5);
    assert_eq!(
        collect_orphan_managed_profile_dirs_at(&paths, &state, future_now),
        vec!["fresh".to_string(), "orphan".to_string()]
    );
}

#[test]
fn runtime_doctor_state_collects_persisted_degradation_and_orphans() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = paths.managed_profiles_root.join("main");
    fs::create_dir_all(&main_home).expect("main home should exist");
    fs::write(main_home.join("auth.json"), "{}").expect("main auth should be written");
    let orphan = paths.managed_profiles_root.join("orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan auth should be written");

    let binding_bound_at = Local::now().timestamp();
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
        response_profile_bindings: BTreeMap::from([(
            "state-resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: binding_bound_at,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "state-session-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: binding_bound_at,
            },
        )]),
    };
    state.save(&paths).expect("state should save");
    let usage_snapshots = BTreeMap::from([(
        "main".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: Local::now().timestamp(),
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 50,
            five_hour_reset_at: 123,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 50,
            weekly_reset_at: 456,
        },
    )]);
    let mut saved_usage_snapshots = false;
    for _ in 0..20 {
        match save_runtime_usage_snapshots(&paths, &usage_snapshots) {
            Ok(()) => {
                saved_usage_snapshots = true;
                break;
            }
            Err(err) => {
                if !err
                    .to_string()
                    .contains("failed to read /tmp/prodex-runtime-test")
                {
                    panic!("usage snapshots should save: {err:#}");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    assert!(saved_usage_snapshots, "usage snapshots should save");
    save_runtime_profile_scores(
        &paths,
        &BTreeMap::from([(
            "__route_bad_pairing__:responses:main".to_string(),
            RuntimeProfileHealth {
                score: 3,
                updated_at: Local::now().timestamp(),
            },
        )]),
    )
    .expect("scores should save");
    save_runtime_profile_backoffs(
        &paths,
        &RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([(
                "main".to_string(),
                Local::now().timestamp() + 30,
            )]),
            transport_backoff_until: BTreeMap::new(),
            route_circuit_open_until: BTreeMap::from([(
                "__route_circuit__:responses:main".to_string(),
                Local::now().timestamp() + 30,
            )]),
        },
    )
    .expect("backoffs should save");
    save_runtime_continuations_for_profiles(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "runtime-resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: binding_bound_at,
                },
            )]),
            session_profile_bindings: BTreeMap::from([(
                "runtime-session-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: binding_bound_at,
                },
            )]),
            turn_state_bindings: BTreeMap::from([(
                "runtime-turn-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: binding_bound_at,
                },
            )]),
            session_id_bindings: BTreeMap::from([(
                "runtime-session-id-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: binding_bound_at,
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
        &state.profiles,
    )
    .expect("runtime continuations should save");
    let journal_saved_at = Local::now().timestamp();
    let recent_not_found_at = journal_saved_at + RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS;
    save_runtime_continuation_journal_for_profiles(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: journal_saved_at,
                },
            )]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Suspect,
                        confidence: 1,
                        last_touched_at: Some(journal_saved_at),
                        last_verified_at: None,
                        last_verified_route: None,
                        last_not_found_at: Some(recent_not_found_at),
                        not_found_streak: 1,
                        success_count: 0,
                        failure_count: 1,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        &state.profiles,
        journal_saved_at,
    )
    .expect("continuation journal should save");

    let _guard = TestEnvVarGuard::set("PRODEX_HOME", paths.root.to_str().unwrap());
    let mut summary = RuntimeDoctorSummary::default();
    collect_runtime_doctor_state(&paths, &mut summary);

    assert_eq!(summary.persisted_retry_backoffs, 1);
    assert_eq!(summary.persisted_route_circuits, 1);
    assert_eq!(summary.persisted_usage_snapshots, 1);
    assert_eq!(summary.persisted_response_bindings, 1);
    assert_eq!(summary.persisted_session_bindings, 1);
    assert_eq!(summary.persisted_turn_state_bindings, 1);
    assert_eq!(summary.persisted_session_id_bindings, 1);
    assert_eq!(summary.persisted_continuation_journal_response_bindings, 1);
    assert_eq!(summary.persisted_suspect_continuations, 1);
    assert_eq!(summary.persisted_dead_continuations, 0);
    assert_eq!(
        summary.continuation_journal_saved_at,
        Some(journal_saved_at)
    );
    assert_eq!(
        summary.suspect_continuation_bindings,
        vec!["resp-main:suspect".to_string()]
    );
    assert_eq!(
        summary.binding_state.active_profile.as_deref(),
        Some("main")
    );
    assert_eq!(summary.binding_state.profile_count, 1);
    assert_eq!(summary.binding_state.state.response_bindings, 1);
    assert_eq!(summary.binding_state.state.session_bindings, 1);
    assert_eq!(
        summary
            .binding_state
            .runtime_continuations
            .response_bindings,
        1
    );
    assert_eq!(
        summary
            .binding_state
            .runtime_continuations
            .turn_state_bindings,
        1
    );
    assert_eq!(
        summary
            .binding_state
            .runtime_continuations
            .session_id_bindings,
        1
    );
    assert_eq!(
        summary.binding_state.continuation_journal.response_bindings,
        1
    );
    assert_eq!(
        summary.binding_state.merged_continuations.response_bindings,
        2
    );
    assert_eq!(
        summary.binding_state.merged_continuations.profiles[0].profile,
        "main"
    );
    let fields = runtime_doctor_fields_for_summary(
        &summary,
        std::path::Path::new("/tmp/prodex-runtime-latest.path"),
    )
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let binding_state = fields
        .get("Binding state")
        .expect("binding state row should render");
    assert!(
        binding_state.contains("active=main profiles=1"),
        "binding state should surface active profile and profile count: {binding_state}"
    );
    assert!(
        binding_state.contains("merged r=2"),
        "binding state should surface merged continuation bindings: {binding_state}"
    );
    assert!(
        summary
            .degraded_routes
            .iter()
            .any(|line| line.contains("main/responses"))
    );
}
