use super::*;

#[test]
fn runtime_doctor_does_not_probe_registry_without_process_identity() {
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

    let server = TinyServer::http("127.0.0.1:0").expect("identity trap server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("identity trap server should expose a TCP address");
    save_runtime_broker_registry(
        &paths,
        "doctor-unproven",
        &RuntimeBrokerRegistry {
            pid: std::process::id(),
            process_birth_identity: None,
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
            realtime_ws_addr: None,
        },
    )
    .expect("unproven broker registry should save");
    save_runtime_broker_test_capability(&paths, "doctor-unproven", "instance", "secret");

    let probed = Arc::new(AtomicBool::new(false));
    let probed_for_thread = Arc::clone(&probed);
    let server_thread = thread::spawn(move || {
        if let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(250)) {
            probed_for_thread.store(true, Ordering::SeqCst);
            let _ = request.respond(TinyResponse::from_string("{}").with_status_code(200));
        }
    });

    let mut summary = RuntimeDoctorSummary::default();
    collect_runtime_doctor_state(&paths, &mut summary);
    server_thread
        .join()
        .expect("identity trap server thread should join");

    assert!(!probed.load(Ordering::SeqCst));
    assert!(
        summary
            .runtime_broker_identities
            .iter()
            .any(|line| line.contains("broker_key=doctor-unproven")),
        "unexpected doctor identities: {:?}",
        summary.runtime_broker_identities
    );
}

#[test]
fn live_broker_metrics_skip_registry_without_process_identity() {
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
    let _prodex_home_guard = TestEnvVarGuard::set("PRODEX_HOME", &paths.root.display().to_string());

    let server = TinyServer::http("127.0.0.1:0").expect("metrics trap server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("metrics trap server should expose a TCP address");
    let registry = RuntimeBrokerRegistry {
        pid: std::process::id(),
        process_birth_identity: None,
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
        realtime_ws_addr: None,
    };
    save_runtime_broker_registry(&paths, "metrics-unproven", &registry)
        .expect("unproven metrics registry should save");
    save_runtime_broker_test_capability(&paths, "metrics-unproven", "instance", "secret");

    let probed = Arc::new(AtomicBool::new(false));
    let probed_for_thread = Arc::clone(&probed);
    let server_thread = thread::spawn(move || {
        if let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(250)) {
            probed_for_thread.store(true, Ordering::SeqCst);
            let _ = request.respond(TinyResponse::from_string("{}").with_status_code(200));
        }
    });

    let observations = collect_live_runtime_broker_observations(&paths);
    server_thread
        .join()
        .expect("metrics trap server thread should join");

    assert!(observations.is_empty());
    assert!(collect_runtime_broker_metrics_targets(&paths).is_empty());
    assert!(!probed.load(Ordering::SeqCst));
}

#[test]
fn runtime_broker_capability_operations_reject_missing_identity_before_http() {
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

    let server = TinyServer::http("127.0.0.1:0").expect("capability trap server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("capability trap server should expose a TCP address");
    let registry = RuntimeBrokerRegistry {
        pid: std::process::id(),
        process_birth_identity: None,
        listen_addr: listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "https://upstream.example".to_string(),
        include_code_review: false,
        upstream_no_proxy: false,
        smart_context_enabled: false,
        current_profile: "main".to_string(),
        instance_id: "instance".to_string(),
        prodex_version: None,
        executable_path: None,
        executable_sha256: None,
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        realtime_ws_addr: None,
    };
    save_runtime_broker_test_capability(&paths, "identity-missing", "instance", "secret");
    let error = runtime_broker_admin_header(&paths, "identity-missing", &registry)
        .expect_err("missing process identity must be rejected before capability loading");
    assert!(error.to_string().contains("process identity"));

    let client = runtime_broker_client_with_config(&RuntimeConfig::compatibility_current())
        .expect("broker client should build");
    let probed = Arc::new(AtomicBool::new(false));
    let probed_for_thread = Arc::clone(&probed);
    let server_thread = thread::spawn(move || {
        if let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(250)) {
            probed_for_thread.store(true, Ordering::SeqCst);
            let _ = request.respond(TinyResponse::from_string("unexpected").with_status_code(200));
        }
    });

    assert!(
        probe_runtime_broker_health(&client, &paths, "identity-missing", &registry)
            .expect("health probe should fail closed")
            .is_none()
    );
    assert!(
        probe_runtime_broker_metrics(&client, &paths, "identity-missing", &registry)
            .expect("metrics probe should fail closed")
            .is_none()
    );
    assert!(
        probe_runtime_broker_log_snapshot(&client, &paths, "identity-missing", &registry, 0)
            .expect("log probe should fail closed")
            .is_none()
    );
    assert!(
        activate_runtime_broker_profile(
            &client,
            &paths,
            "identity-missing",
            &registry,
            "main",
        )
        .is_err()
    );
    assert!(
        release_runtime_broker_session_affinity(
            &client,
            &paths,
            "identity-missing",
            &registry,
            "session",
        )
        .is_err()
    );
    assert!(
        send_runtime_broker_log_event(
            &client,
            &paths,
            "identity-missing",
            &registry,
            "message",
        )
        .is_err()
    );

    server_thread
        .join()
        .expect("capability trap server thread should join");
    assert!(!probed.load(Ordering::SeqCst));
}
