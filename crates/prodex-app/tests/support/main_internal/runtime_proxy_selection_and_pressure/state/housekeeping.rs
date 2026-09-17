use super::*;

#[path = "housekeeping_auto.rs"]
mod auto;

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
                    plan_type: None,
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
                    plan_type: None,
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
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");
    let stale_login = paths.managed_profiles_root.join(".login-123-1-0");
    fs::create_dir_all(&stale_login).expect("stale login dir should exist");

    let simulated_now = SystemTime::now()
        .checked_add(Duration::from_secs(
            (PROD_EX_TMP_LOGIN_RETENTION_SECONDS + 5).max(1) as u64,
        ))
        .expect("simulated clock should be valid");
    cleanup_stale_login_dirs_at(&paths, simulated_now);

    assert!(!stale_login.exists());
}
