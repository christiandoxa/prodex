use super::*;

#[test]
fn auto_runtime_housekeeping_removes_runtime_garbage_without_touching_user_state() {
    let temp_dir = TestDir::isolated();
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
    fs::create_dir_all(&paths.shared_codex_root).expect("shared codex root should exist");

    for index in 0..=RUNTIME_PROXY_LOG_RETENTION_COUNT {
        let path = runtime_log_dir.join(format!(
            "{RUNTIME_PROXY_LOG_FILE_PREFIX}-auto-cleanup-{index}.log"
        ));
        fs::write(path, "log").expect("runtime log should write");
    }
    let pointer = runtime_log_dir.join(RUNTIME_PROXY_LATEST_LOG_POINTER);
    fs::write(
        &pointer,
        format!("{}\n", runtime_log_dir.join("missing.log").display()),
    )
    .expect("pointer should write");

    let stale_login = paths.root.join(".login-456-1-0");
    fs::create_dir_all(&stale_login).expect("stale login dir should exist");
    let stale_root_temp = paths.root.join("runtime-scores.json.999999999.1.0.tmp");
    fs::write(&stale_root_temp, "tmp").expect("stale root temp should write");

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

    fs::write(
        paths.shared_codex_root.join("history.jsonl"),
        "{\"session_id\":\"old\",\"text\":\"keep\"}\n",
    )
    .expect("shared history should write");
    let old_session_dir = paths.shared_codex_root.join("sessions/2026/04/10");
    fs::create_dir_all(&old_session_dir).expect("old codex session dir should exist");
    fs::write(old_session_dir.join("old.jsonl"), "{}\n").expect("old session should write");

    let orphan = paths.managed_profiles_root.join("orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan auth should write");

    let stale_broker_key = "auto-cleanup-stale";
    save_runtime_broker_registry(
        &paths,
        stale_broker_key,
        &RuntimeBrokerRegistry {
            pid: 999_999_999,
            listen_addr: "127.0.0.1:1".to_string(),
            started_at: 1,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            smart_context_enabled: false,
            current_profile: "main".to_string(),
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
    fs::write(&stale_lease, "stale").expect("stale lease should write");

    let future_offset_seconds =
        (RUNTIME_PROXY_LOG_RETENTION_SECONDS.max(PROD_EX_TMP_LOGIN_RETENTION_SECONDS) + 5) as u64;
    let simulated_now = SystemTime::now()
        .checked_add(Duration::from_secs(future_offset_seconds))
        .expect("simulated clock should be valid");
    let summary = run_prodex_auto_runtime_housekeeping_for_paths_at(
        &paths,
        &runtime_log_dir,
        &pointer,
        simulated_now,
        0,
    )
    .expect("auto housekeeping should succeed")
    .expect("auto housekeeping should run");

    assert_eq!(summary.runtime_logs_removed, RUNTIME_PROXY_LOG_RETENTION_COUNT + 1);
    assert_eq!(summary.stale_runtime_log_pointer_removed, 1);
    assert_eq!(summary.stale_login_dirs_removed, 1);
    assert_eq!(summary.stale_root_temp_files_removed, 1);
    assert_eq!(summary.dead_runtime_broker_leases_removed, 1);
    assert_eq!(summary.dead_runtime_broker_registries_removed, 1);
    assert_eq!(summary.orphan_managed_profile_dirs_removed, 0);
    assert_eq!(summary.transient_root_files_removed, 0);

    assert!(!pointer.exists(), "stale runtime pointer should be removed");
    assert!(!stale_login.exists(), "stale login dir should be removed");
    assert!(!stale_root_temp.exists(), "stale root temp file should be removed");
    assert!(
        !runtime_broker_registry_file_path(&paths, stale_broker_key).exists(),
        "dead broker registry should be removed"
    );
    assert!(!stale_lease.exists(), "dead broker lease should be removed");
    assert!(orphan.exists(), "orphan managed home should remain manual cleanup");
    assert!(old_session_dir.join("old.jsonl").exists());
    assert!(paths.shared_codex_root.join("history.jsonl").exists());
    for path in &transient_root_files {
        assert!(path.exists());
    }

    let skipped = run_prodex_auto_runtime_housekeeping_for_paths_at(
        &paths,
        &runtime_log_dir,
        &pointer,
        simulated_now,
        AUTO_RUNTIME_HOUSEKEEPING_INTERVAL_SECONDS.max(60),
    )
    .expect("throttled auto housekeeping should succeed");
    assert!(skipped.is_none(), "recent auto housekeeping should be throttled");
}
