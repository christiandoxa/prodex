use super::*;

#[path = "housekeeping_auto.rs"]
mod auto;
#[path = "housekeeping_chat_history.rs"]
mod chat_history;
#[path = "housekeeping_profile_dedupe.rs"]
mod profile_dedupe;

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
            updated_at: BTreeMap::new(),
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
            instance_id: "stale-instance".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
            openai_mount_path: None,
        },
    )
    .expect("stale runtime broker registry should save");
    save_runtime_broker_test_capability(&paths, stale_broker_key, "stale-instance", "stale-admin");
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
    assert!(
        !runtime_broker_capability_file_path(&paths, stale_broker_key).exists(),
        "stale runtime broker capability should be removed"
    );
    assert!(!stale_lease.exists(), "dead lease should be removed");
    assert!(
        !lease_only_stale.exists(),
        "dead lease without registry should be removed"
    );
    assert!(live_lease.exists(), "live lease should remain");
}
