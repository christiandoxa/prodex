use super::*;

#[test]
fn runtime_sidecar_housekeeping_prunes_stale_entries() {
    let temp_dir = TestDir::isolated();
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
    let temp_dir = TestDir::isolated();
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
    let temp_dir = TestDir::isolated();
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

    for index in 0..=RUNTIME_PROXY_LOG_RETENTION_COUNT {
        let path = runtime_log_dir.join(format!(
            "{RUNTIME_PROXY_LOG_FILE_PREFIX}-cleanup-{index}.log"
        ));
        fs::write(path, "log").expect("runtime log should write");
    }
    let pointer = runtime_log_dir.join(RUNTIME_PROXY_LATEST_LOG_POINTER);
    let missing_log = runtime_log_dir.join("missing.log");
    fs::write(&pointer, format!("{}\n", missing_log.display())).expect("pointer should write");

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
            upstream_no_proxy: false,
        smart_context_enabled: false,
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

#[test]
fn perform_prodex_cleanup_leaves_codex_managed_chat_history_untouched() {
    let temp_dir = TestDir::isolated();
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
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .expect("fixed timestamp should be valid")
        .timestamp();
    let old_ts = now - (31 * 24 * 60 * 60);
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
    let recent_session_dir = paths.shared_codex_root.join("sessions/2026/05/18");
    fs::create_dir_all(&old_session_dir).expect("old codex session dir should exist");
    fs::create_dir_all(&recent_session_dir).expect("recent codex session dir should exist");
    fs::write(
        old_session_dir.join("old.jsonl"),
        format!(
            "{{\"timestamp\":\"{}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\"}}}}\n",
            Local.timestamp_opt(old_ts, 0).single().unwrap().to_rfc3339()
        ),
    )
    .expect("old codex session should write");
    fs::write(
        recent_session_dir.join("recent.jsonl"),
        format!(
            "{{\"timestamp\":\"{}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\"}}}}\n",
            Local
                .timestamp_opt(recent_ts, 0)
                .single()
                .unwrap()
                .to_rfc3339()
        ),
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
        format!(
            "{{\"timestamp\":\"{}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\"}}}}\n",
            Local.timestamp_opt(old_ts, 0).single().unwrap().to_rfc3339()
        ),
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

    assert_eq!(summary.total_removed(), 0);
    let codex_history = fs::read_to_string(paths.shared_codex_root.join("history.jsonl"))
        .expect("shared codex history should remain");
    assert!(codex_history.contains("\"session_id\":\"old\""));
    assert!(codex_history.contains("\"session_id\":\"recent\""));
    assert!(codex_history.contains("\"session_id\":\"unstamped\""));
    assert!(
        old_session_dir.join("old.jsonl").exists(),
        "old dated codex session should remain Codex-managed"
    );
    assert!(
        recent_session_dir.join("recent.jsonl").exists(),
        "recent dated codex session should remain"
    );
    assert!(
        profile_old_session_dir.join("profile-old.json").exists(),
        "old managed profile session should remain Codex-managed"
    );

    let claude_history = fs::read_to_string(shared_claude_project.join("session.jsonl"))
        .expect("shared claude history should remain");
    assert!(claude_history.contains("\"message\":\"old\""));
    assert!(claude_history.contains("\"message\":\"recent\""));
}

#[test]
fn perform_prodex_cleanup_deduplicates_profiles_by_email() {
    let temp_dir = TestDir::isolated();
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
fn perform_prodex_cleanup_keeps_same_email_profiles_when_workspace_differs() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");

    let first_home = paths.managed_profiles_root.join("first");
    let second_home = paths.managed_profiles_root.join("second");
    fs::create_dir_all(&first_home).expect("first home should exist");
    fs::create_dir_all(&second_home).expect("second home should exist");
    fs::write(
        first_home.join("auth.json"),
        r#"{"tokens":{"access_token":"token-one","account_id":"acct-one"}}"#,
    )
    .expect("first auth should write");
    fs::write(
        second_home.join("auth.json"),
        r#"{"tokens":{"access_token":"token-two","account_id":"acct-two"}}"#,
    )
    .expect("second auth should write");

    let mut state = AppState {
        active_profile: Some("first".to_string()),
        profiles: BTreeMap::from([
            (
                "first".to_string(),
                ProfileEntry {
                    codex_home: first_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: true,
                    email: Some("Main@Example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        ..AppState::default()
    };

    let summary = perform_prodex_cleanup(&paths, &mut state).expect("cleanup should succeed");

    assert_eq!(summary.duplicate_profiles_removed, 0);
    assert_eq!(state.profiles.len(), 2);
    assert!(first_home.exists());
    assert!(second_home.exists());
}
