#[path = "../test_wait.rs"]
mod test_wait;

use std::process::Child;
use std::os::unix::fs::PermissionsExt;

use super::*;
use test_wait::wait_for_poll;

fn wait_for_runtime_process_row(pid: u32) {
    wait_for_poll(
        "runtime broker process to appear",
        Duration::from_secs(2),
        Duration::from_millis(10),
        || {
            collect_process_rows()
                .into_iter()
                .any(|row| row.pid == pid)
                .then_some(())
        },
    );
}

fn wait_for_runtime_process_alive(pid: u32) {
    wait_for_poll(
        "runtime broker process to become alive",
        Duration::from_secs(2),
        Duration::from_millis(10),
        || runtime_process_pid_alive(pid).then_some(()),
    );
}

fn wait_for_runtime_child_exit(child: &mut Child) {
    wait_for_poll(
        "runtime broker child to exit",
        Duration::from_secs(2),
        Duration::from_millis(20),
        || match child.try_wait().expect("child wait should succeed") {
            Some(_) => Some(()),
            None => None,
        },
    );
}

#[test]
fn runtime_broker_openai_mount_path_falls_back_to_running_legacy_broker_version() {
    let temp_dir = TestDir::isolated();
    let script_path = temp_dir.path.join("legacy-prodex.sh");
    fs::write(
        &script_path,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'prodex 0.2.99'\n  exit 0\nfi\nsleep 30\n",
    )
    .expect("legacy broker script should write");
    let mut permissions = fs::metadata(&script_path)
        .expect("legacy broker script metadata should load")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions)
        .expect("legacy broker script permissions should update");

    let mut child = Command::new(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("legacy broker script should spawn");

    wait_for_runtime_process_row(child.id());

    let registry = RuntimeBrokerRegistry {
        pid: child.id(),
        listen_addr: "127.0.0.1:9".to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        instance_token: "instance".to_string(),
        admin_token: "secret".to_string(),
        prodex_version: None,
        executable_path: None,
        executable_sha256: None,
        openai_mount_path: None,
    };

    let mount_path = runtime_broker_openai_mount_path(&registry)
        .expect("legacy broker mount path should resolve from running broker version");

    let _ = child.kill();
    let _ = child.wait();

    assert_eq!(mount_path, "/backend-api/prodex/v0.2.99");
}

#[test]
fn wait_for_existing_runtime_broker_recovery_or_exit_replaces_mismatched_live_broker() {
    let _timeout_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "500");
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "replace-version-mismatch";
    let script_path = temp_dir.path.join("mismatched-broker.sh");
    fs::write(
        &script_path,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'prodex 0.0.1'\n  exit 0\nfi\nsleep 30\n",
    )
    .expect("mismatched broker script should write");
    let mut permissions = fs::metadata(&script_path)
        .expect("mismatched broker script metadata should load")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions)
        .expect("mismatched broker script permissions should update");

    let mut child = Command::new(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("mismatched broker script should spawn");

    wait_for_runtime_process_alive(child.id());

    let registry = RuntimeBrokerRegistry {
        pid: child.id(),
        listen_addr: "127.0.0.1:9".to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "http://127.0.0.1:12345/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        instance_token: "instance".to_string(),
        admin_token: "secret".to_string(),
        prodex_version: None,
        executable_path: None,
        executable_sha256: None,
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
    };
    save_runtime_broker_registry(&paths, broker_key, &registry)
        .expect("mismatched broker registry should save");
    let client = runtime_broker_client().expect("broker client should build");

    let recovered = wait_for_existing_runtime_broker_recovery_or_exit(
        &client,
        &paths,
        broker_key,
        &registry.upstream_base_url,
        registry.include_code_review,
    )
    .expect("wait should not fail");

    wait_for_runtime_child_exit(&mut child);

    assert!(
        recovered.is_none(),
        "mismatched broker version should not be reused"
    );
    assert!(
        !runtime_process_pid_alive(registry.pid),
        "mismatched live broker should be terminated during replacement"
    );
    assert!(
        load_runtime_broker_registry(&paths, broker_key)
            .expect("registry reload should succeed")
            .is_none(),
        "terminated mismatched broker should clear its registry"
    );
}

#[test]
fn wait_for_existing_runtime_broker_recovery_or_exit_yields_mismatched_live_broker_with_active_requests()
{
    let _timeout_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "2000");
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "defer-version-mismatch";
    let script_path = temp_dir.path.join("busy-mismatched-broker.sh");
    fs::write(
        &script_path,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'prodex 0.0.1'\n  exit 0\nfi\nsleep 30\n",
    )
    .expect("busy mismatched broker script should write");
    let mut permissions = fs::metadata(&script_path)
        .expect("busy mismatched broker script metadata should load")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions)
        .expect("busy mismatched broker script permissions should update");

    let mut child = Command::new(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("busy mismatched broker script should spawn");
    let child_pid = child.id();
    wait_for_runtime_process_alive(child_pid);

    let server = TinyServer::http("127.0.0.1:0").expect("busy health server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("busy health server should expose a TCP address");
    let health_thread = thread::spawn(move || {
        while let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(250)) {
            let body = serde_json::to_string(&RuntimeBrokerHealth {
                pid: child_pid,
                started_at: Local::now().timestamp(),
                current_profile: "main".to_string(),
                include_code_review: false,
                active_requests: 2,
                instance_token: "instance".to_string(),
                persistence_role: "owner".to_string(),
                prodex_version: Some("0.0.1".to_string()),
                executable_path: Some("/tmp/busy-mismatched-broker.sh".to_string()),
                executable_sha256: None,
            })
            .expect("busy health response should serialize");
            let response = TinyResponse::from_string(body).with_status_code(200);
            request
                .respond(response)
                .expect("busy health response should send");
        }
    });

    let registry = RuntimeBrokerRegistry {
        pid: child_pid,
        listen_addr: listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "http://127.0.0.1:12345/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        instance_token: "instance".to_string(),
        admin_token: "secret".to_string(),
        prodex_version: Some("0.0.1".to_string()),
        executable_path: Some(script_path.display().to_string()),
        executable_sha256: None,
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
    };
    save_runtime_broker_registry(&paths, broker_key, &registry)
        .expect("busy mismatched broker registry should save");
    let client = runtime_broker_client().expect("broker client should build");

    let started_at = Instant::now();
    let recovered = wait_for_existing_runtime_broker_recovery_or_exit(
        &client,
        &paths,
        broker_key,
        &registry.upstream_base_url,
        registry.include_code_review,
    )
    .expect("wait should not fail");
    let elapsed = started_at.elapsed();

    assert!(
        recovered.is_none(),
        "busy mismatched broker should not be reused"
    );
    assert!(
        runtime_process_pid_alive(registry.pid),
        "busy mismatched broker should stay alive while it still serves active requests"
    );
    assert!(
        elapsed < Duration::from_millis(1_000),
        "launcher should yield quickly to spawn a current broker instead of waiting for a busy stale broker: {elapsed:?}"
    );
    assert!(
        load_runtime_broker_registry(&paths, broker_key)
            .expect("registry reload should succeed")
            .is_some(),
        "busy mismatched broker registry should remain until the session drains"
    );

    let _ = child.kill();
    let _ = child.wait();
    health_thread
        .join()
        .expect("busy mismatched health thread should join");
}

#[test]
fn find_compatible_runtime_broker_registry_discovers_other_broker_key() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let script_path = temp_dir.path.join("current-version-broker.sh");
    fs::write(
        &script_path,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'prodex {}'\n  exit 0\nfi\nsleep 30\n",
            runtime_current_prodex_version()
        ),
    )
    .expect("current-version broker script should write");
    let mut permissions = fs::metadata(&script_path)
        .expect("current-version broker script metadata should load")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions)
        .expect("current-version broker script permissions should update");
    let mut child = Command::new(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("current-version broker script should spawn");
    wait_for_runtime_process_alive(child.id());

    let server = TinyServer::http("127.0.0.1:0").expect("health server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("health server should expose a TCP address");
    let registry = RuntimeBrokerRegistry {
        pid: child.id(),
        listen_addr: listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        instance_token: "legacy-instance".to_string(),
        admin_token: "secret".to_string(),
        prodex_version: None,
        executable_path: None,
        executable_sha256: None,
        openai_mount_path: Some("/backend-api/prodex/v0.2.99".to_string()),
    };
    save_runtime_broker_registry(&paths, "legacy-key", &registry)
        .expect("legacy broker registry should save");

    let health_thread = thread::spawn(move || {
        let request = server.recv().expect("health request should arrive");
        let body = serde_json::to_string(&RuntimeBrokerHealth {
            pid: std::process::id(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            active_requests: 0,
            instance_token: "legacy-instance".to_string(),
            persistence_role: "owner".to_string(),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: None,
            executable_sha256: None,
        })
        .expect("health payload should serialize");
        let response = TinyResponse::from_string(body).with_status_code(200);
        request
            .respond(response)
            .expect("health response should write");
    });
    let client = runtime_broker_client().expect("broker client should build");

    let discovered = find_compatible_runtime_broker_registry(
        &client,
        &paths,
        "current-key",
        "https://chatgpt.com/backend-api",
        false,
    )
    .expect("registry scan should not fail")
    .expect("compatible registry should be discovered");

    health_thread
        .join()
        .expect("health server thread should join");
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(discovered.0, "legacy-key");
    assert_eq!(discovered.1.instance_token, "legacy-instance");
}

#[test]
fn wait_for_existing_runtime_broker_recovery_or_exit_yields_after_live_unhealthy_registry_clears() {
    let _timeout_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "500");
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "wait-test";
    let script_path = temp_dir.path.join("unhealthy-current-broker.sh");
    fs::write(
        &script_path,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'prodex {}'\n  exit 0\nfi\nsleep 30\n",
            runtime_current_prodex_version()
        ),
    )
    .expect("unhealthy current broker script should write");
    let mut permissions = fs::metadata(&script_path)
        .expect("unhealthy current broker script metadata should load")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions)
        .expect("unhealthy current broker script permissions should update");
    let mut child = Command::new(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("unhealthy current broker script should spawn");
    wait_for_runtime_process_alive(child.id());

    let registry = RuntimeBrokerRegistry {
        pid: child.id(),
        listen_addr: "127.0.0.1:9".to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "http://127.0.0.1:12345/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        instance_token: "instance".to_string(),
        admin_token: "secret".to_string(),
        prodex_version: None,
        executable_path: None,
        executable_sha256: None,
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
    };
    save_runtime_broker_registry(&paths, broker_key, &registry)
        .expect("registry should save for wait test");

    let paths_for_clear = paths.clone();
    let instance_token = registry.instance_token.clone();
    let upstream_base_url = registry.upstream_base_url.clone();
    let include_code_review = registry.include_code_review;
    let clear_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(75));
        remove_runtime_broker_registry_if_token_matches(
            &paths_for_clear,
            broker_key,
            &instance_token,
        );
    });
    let client = runtime_broker_client().expect("broker client should build");

    let recovered = wait_for_existing_runtime_broker_recovery_or_exit(
        &client,
        &paths,
        broker_key,
        &upstream_base_url,
        include_code_review,
    )
    .expect("wait should not fail");

    clear_thread
        .join()
        .expect("registry clear thread should join");
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        recovered.is_none(),
        "wait should yield once the live unhealthy registry clears"
    );
}

#[test]
fn wait_for_existing_runtime_broker_recovery_or_exit_clears_dead_registry_without_probe() {
    let _timeout_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "200");
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "dead-registry-fast-clear";
    let server = TinyServer::http("127.0.0.1:0").expect("dead registry probe server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("dead registry probe server should expose a TCP address");
    let probed = Arc::new(AtomicBool::new(false));
    let probed_for_thread = Arc::clone(&probed);
    let server_thread = thread::spawn(move || {
        if let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(250)) {
            probed_for_thread.store(true, Ordering::SeqCst);
            let _ = request.respond(TinyResponse::from_string("unexpected probe"));
        }
    });

    let registry = RuntimeBrokerRegistry {
        pid: 999_999_999,
        listen_addr: listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        instance_token: "dead-instance".to_string(),
        admin_token: "secret".to_string(),
        prodex_version: None,
        executable_path: None,
        executable_sha256: None,
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
    };
    save_runtime_broker_registry(&paths, broker_key, &registry)
        .expect("dead broker registry should save");
    let client = runtime_broker_client().expect("broker client should build");

    let recovered = wait_for_existing_runtime_broker_recovery_or_exit(
        &client,
        &paths,
        broker_key,
        &registry.upstream_base_url,
        registry.include_code_review,
    )
    .expect("wait should not fail");

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

#[test]
fn find_compatible_runtime_broker_registry_prunes_dead_registry_without_probe() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "dead-compatible-key";
    let server =
        TinyServer::http("127.0.0.1:0").expect("dead compatible probe server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("dead compatible probe server should expose a TCP address");
    let probed = Arc::new(AtomicBool::new(false));
    let probed_for_thread = Arc::clone(&probed);
    let server_thread = thread::spawn(move || {
        if let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(250)) {
            probed_for_thread.store(true, Ordering::SeqCst);
            let _ = request.respond(TinyResponse::from_string("unexpected probe"));
        }
    });

    let registry = RuntimeBrokerRegistry {
        pid: 999_999_999,
        listen_addr: listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        instance_token: "dead-compatible".to_string(),
        admin_token: "secret".to_string(),
        prodex_version: Some(runtime_current_prodex_version().to_string()),
        executable_path: None,
        executable_sha256: None,
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
    };
    save_runtime_broker_registry(&paths, broker_key, &registry)
        .expect("dead compatible registry should save");
    let client = runtime_broker_client().expect("broker client should build");

    let discovered = find_compatible_runtime_broker_registry(
        &client,
        &paths,
        "excluded-key",
        &registry.upstream_base_url,
        registry.include_code_review,
    )
    .expect("compatible scan should not fail");

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
