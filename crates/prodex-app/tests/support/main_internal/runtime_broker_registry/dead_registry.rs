use super::*;

fn start_health_probe_trap(
    server: Arc<TinyServer>,
    registry: &RuntimeBrokerRegistry,
) -> (Arc<AtomicBool>, JoinHandle<()>) {
    let probed = Arc::new(AtomicBool::new(false));
    let probed_for_thread = Arc::clone(&probed);
    let health = registry.clone();
    let server_thread = thread::spawn(move || {
        let Ok(request) = server.recv() else {
            return;
        };
        probed_for_thread.store(true, Ordering::SeqCst);
        let body = serde_json::to_string(&RuntimeBrokerHealth {
            pid: health.pid,
            started_at: health.started_at,
            current_profile: health.current_profile,
            include_code_review: health.include_code_review,
            active_requests: 0,
            instance_id: health.instance_id,
            persistence_role: "owner".to_string(),
            prodex_version: health.prodex_version,
            executable_path: health.executable_path,
            executable_sha256: health.executable_sha256,
        })
        .expect("health body should serialize");
        let _ = request.respond(TinyResponse::from_string(body));
    });
    (probed, server_thread)
}

pub(super) fn wait_for_existing_runtime_broker_recovery_or_exit_clears_dead_registry_without_probe()
{
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "dead-registry-fast-clear";
    let server =
        Arc::new(TinyServer::http("127.0.0.1:0").expect("dead registry probe server should bind"));
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("dead registry probe server should expose a TCP address");

    let registry = RuntimeBrokerRegistry {
        pid: 999_999_999,
        process_birth_identity: None,
        listen_addr: listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        upstream_no_proxy: false,
        smart_context_enabled: false,
        current_profile: "main".to_string(),
        instance_id: "dead-instance".to_string(),
        prodex_version: None,
        executable_path: None,
        executable_sha256: None,
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        realtime_ws_addr: None,
    };
    save_runtime_broker_registry(&paths, broker_key, &registry)
        .expect("dead broker registry should save");
    save_runtime_broker_test_capability(&paths, broker_key, &registry.instance_id, "test-secret");
    let (probed, server_thread) = start_health_probe_trap(Arc::clone(&server), &registry);
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .build()
        .expect("broker client should build");

    let recovered = wait_for_existing_runtime_broker_recovery_or_exit(
        &client,
        &paths,
        broker_key,
        &registry.upstream_base_url,
        registry.include_code_review,
        registry.upstream_no_proxy,
    )
    .expect("wait should not fail");

    server.unblock();
    server_thread
        .join()
        .expect("dead registry probe server should join");

    assert!(
        recovered.is_none(),
        "dead broker registry should be discarded instead of reused"
    );
    assert!(
        !probed.load(Ordering::SeqCst),
        "dead broker registry should not trigger a health probe"
    );
    assert!(
        load_runtime_broker_registry(&paths, broker_key)
            .expect("registry reload should succeed")
            .is_none(),
        "dead broker registry should be cleared"
    );
}

pub(super) fn find_compatible_runtime_broker_registry_prunes_dead_registry_without_probe() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "dead-compatible-key";
    let server = Arc::new(
        TinyServer::http("127.0.0.1:0").expect("dead compatible probe server should bind"),
    );
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("dead compatible probe server should expose a TCP address");

    let registry = RuntimeBrokerRegistry {
        pid: 999_999_999,
        process_birth_identity: None,
        listen_addr: listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        upstream_no_proxy: false,
        smart_context_enabled: false,
        current_profile: "main".to_string(),
        instance_id: "dead-compatible".to_string(),
        prodex_version: Some(runtime_current_prodex_version().to_string()),
        executable_path: None,
        executable_sha256: None,
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        realtime_ws_addr: None,
    };
    save_runtime_broker_registry(&paths, broker_key, &registry)
        .expect("dead compatible registry should save");
    save_runtime_broker_test_capability(&paths, broker_key, &registry.instance_id, "test-secret");
    let (probed, server_thread) = start_health_probe_trap(Arc::clone(&server), &registry);
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .build()
        .expect("broker client should build");

    let discovered = find_compatible_runtime_broker_registry(
        &client,
        &paths,
        "excluded-key",
        &registry.upstream_base_url,
        registry.include_code_review,
        registry.upstream_no_proxy,
    )
    .expect("compatible scan should not fail");

    server.unblock();
    server_thread
        .join()
        .expect("dead compatible probe server should join");

    assert!(
        discovered.is_none(),
        "dead compatible registry should not be returned"
    );
    assert!(
        !probed.load(Ordering::SeqCst),
        "dead compatible registry should be pruned before health probing"
    );
    assert!(
        load_runtime_broker_registry(&paths, broker_key)
            .expect("registry reload should succeed")
            .is_none(),
        "dead compatible registry should be removed from disk"
    );
}
