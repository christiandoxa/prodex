use super::*;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use crate::TestEnvVarGuard;

include!("main_internal/runtime_test_auth.rs");
include!("main_internal/runtime_test_support.rs");
include!("main_internal/runtime_test_websocket.rs");
include!("main_internal/runtime_proxy_backend.rs");
include!("main_internal/runtime_proxy_continuation_helpers.rs");

#[test]
fn validates_profile_names() {
    assert!(validate_profile_name("alpha-1").is_ok());
    assert!(validate_profile_name("bad/name").is_err());
    assert!(validate_profile_name("bad space").is_err());
}

#[test]
fn recognizes_known_windows() {
    assert_eq!(window_label(Some(18_000)), "5h");
    assert_eq!(window_label(Some(604_800)), "weekly");
    assert_eq!(window_label(Some(2_592_000)), "monthly");
}

#[test]
fn blocks_when_main_window_is_exhausted() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(100),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: None,
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

    let blocked = collect_blocked_limits(&usage, false);
    assert_eq!(blocked.len(), 2);
    assert!(blocked[0].message.starts_with("5h exhausted until "));
    assert_eq!(blocked[1].message, "weekly quota unavailable");
}

#[test]
fn blocks_when_weekly_window_is_missing() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: None,
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

    let blocked = collect_blocked_limits(&usage, false);
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].message, "weekly quota unavailable");
}

#[test]
fn compact_window_format_uses_scale_of_100() {
    let window = UsageWindow {
        used_percent: Some(37),
        reset_at: None,
        limit_window_seconds: Some(18_000),
    };

    assert_eq!(format_window_status_compact(&window), "5h 63% left");
    assert!(format_window_status(&window).contains("63% left"));
    assert!(format_window_status(&window).contains("37% used"));
}

#[test]
fn main_reset_summary_lists_required_windows() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(30),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

    let summary = format_main_reset_summary(&usage);
    assert!(summary.starts_with("5h "));
    assert!(summary.contains(" | weekly "));
    assert!(summary.contains(&format_precise_reset_time(Some(1_700_000_000))));
}

#[test]
fn main_reset_summary_marks_missing_required_window() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: None,
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

    assert_eq!(
        format_main_reset_summary(&usage),
        format!(
            "5h {} | weekly unavailable",
            format_precise_reset_time(Some(1_700_000_000))
        )
    );
}

#[test]
fn map_parallel_runs_jobs_concurrently_and_preserves_order() {
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(Mutex::new(0usize));
    let started = Instant::now();

    let output = map_parallel(vec![1, 2, 3, 4], {
        let active = Arc::clone(&active);
        let max_active = Arc::clone(&max_active);
        move |value| {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            {
                let mut seen_max = max_active.lock().expect("max_active poisoned");
                *seen_max = (*seen_max).max(current);
            }

            thread::sleep(Duration::from_millis(50));
            active.fetch_sub(1, Ordering::SeqCst);
            value * 10
        }
    });

    assert_eq!(output, vec![10, 20, 30, 40]);
    assert!(
        *max_active.lock().expect("max_active poisoned") >= 2,
        "parallel worker count never exceeded one"
    );
    assert!(
        started.elapsed() < Duration::from_millis(150),
        "parallel execution took too long: {:?}",
        started.elapsed()
    );
}

#[test]
fn ready_profile_ranking_prefers_soon_recovering_weekly_capacity() {
    let candidates = vec![
        ReadyProfileCandidate {
            name: "slow".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "fast".to_string(),
            usage: usage_with_main_windows(80, 18_000, 80, 86_400),
            order_index: 1,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let mut ranked = candidates.clone();
    ranked.sort_by_key(ready_profile_sort_key);
    assert_eq!(ranked[0].name, "fast");
}

#[test]
fn runtime_probe_cache_freshness_distinguishes_fresh_stale_and_expired() {
    let now = Local::now().timestamp();
    let fresh = RuntimeProfileProbeCacheEntry {
        checked_at: now,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
    };
    let stale = RuntimeProfileProbeCacheEntry {
        checked_at: now - (RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS + 1),
        auth: fresh.auth.clone(),
        result: fresh.result.clone(),
    };
    let expired = RuntimeProfileProbeCacheEntry {
        checked_at: now - (RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS + 1),
        auth: fresh.auth.clone(),
        result: fresh.result.clone(),
    };

    assert_eq!(
        runtime_profile_probe_cache_freshness(&fresh, now),
        RuntimeProbeCacheFreshness::Fresh
    );
    assert_eq!(
        runtime_profile_probe_cache_freshness(&stale, now),
        RuntimeProbeCacheFreshness::StaleUsable
    );
    assert_eq!(
        runtime_profile_probe_cache_freshness(&expired, now),
        RuntimeProbeCacheFreshness::Expired
    );
}

#[test]
fn startup_probe_refresh_targets_current_then_stale_or_missing_profiles() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    let fourth_home = temp_dir.path.join("homes/fourth");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");
    write_auth_json(&fourth_home.join("auth.json"), "fourth-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "fourth".to_string(),
                ProfileEntry {
                    codex_home: fourth_home,
                    managed: true,
                    email: Some("fourth@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
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
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let probe_cache = BTreeMap::from([(
        "fourth".to_string(),
        RuntimeProfileProbeCacheEntry {
            checked_at: now,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 18_000, 90, 604_800)),
        },
    )]);
    let usage_snapshots = BTreeMap::from([(
        "second".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: now,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 85,
            five_hour_reset_at: now + 18_000,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 92,
            weekly_reset_at: now + 604_800,
        },
    )]);

    assert_eq!(
        runtime_profiles_needing_startup_probe_refresh(
            &state,
            "third",
            &probe_cache,
            &usage_snapshots,
            now,
        ),
        vec!["third".to_string(), "main".to_string()]
    );
}

#[test]
fn startup_probe_refresh_warms_current_profiles_when_snapshots_are_empty() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
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
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    assert_eq!(
        runtime_profiles_needing_startup_probe_refresh(
            &state,
            "second",
            &BTreeMap::new(),
            &BTreeMap::new(),
            Local::now().timestamp(),
        ),
        vec![
            "second".to_string(),
            "third".to_string(),
            "main".to_string()
        ]
    );
}


include!("main_internal/runtime_proxy_selection_and_pressure.rs");

#[test]
fn version_is_newer_compares_semver_like_versions() {
    assert!(version_is_newer("0.2.47", "0.2.46"));
    assert!(version_is_newer("1.0.0", "0.9.9"));
    assert!(!version_is_newer("0.2.46", "0.2.46"));
    assert!(!version_is_newer("0.2.45", "0.2.46"));
}

#[test]
fn prodex_update_command_prefers_cargo_for_native_installations() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    assert_eq!(
        prodex_update_command_for_version("0.2.99"),
        "cargo install prodex --force --version 0.2.99"
    );
}

#[test]
fn prodex_update_command_prefers_npm_for_npm_installations() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "@christiandoxa/prodex");
    assert_eq!(
        prodex_update_command_for_version("0.2.99"),
        "npm install -g @christiandoxa/prodex@0.2.99 or npm install -g @christiandoxa/prodex@latest"
    );
}

#[test]
fn current_prodex_release_source_is_npm_when_wrapped_by_npm() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "@christiandoxa/prodex");
    assert_eq!(current_prodex_release_source(), ProdexReleaseSource::Npm);
}

#[test]
fn current_prodex_release_source_defaults_to_crates_io() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    assert_eq!(
        current_prodex_release_source(),
        ProdexReleaseSource::CratesIo
    );
}

#[test]
fn cached_update_version_is_scoped_to_release_source() {
    let now = Local::now().timestamp();
    assert!(!should_use_cached_update_version(
        ProdexReleaseSource::CratesIo,
        "0.2.99",
        now,
        ProdexReleaseSource::Npm,
        current_prodex_version(),
        now
    ));
    assert!(should_use_cached_update_version(
        ProdexReleaseSource::Npm,
        "0.2.99",
        now,
        ProdexReleaseSource::Npm,
        current_prodex_version(),
        now
    ));
}

#[test]
fn update_notice_is_suppressed_for_machine_output_modes() {
    assert!(!should_emit_update_notice(&Commands::Info(InfoArgs {})));
    assert!(!should_emit_update_notice(&Commands::Doctor(DoctorArgs {
        quota: false,
        runtime: true,
        json: true,
    })));
    assert!(!should_emit_update_notice(&Commands::Audit(AuditArgs {
        tail: 20,
        json: true,
        component: None,
        action: None,
        outcome: None,
    })));
    assert!(!should_emit_update_notice(&Commands::Quota(QuotaArgs {
        profile: None,
        all: false,
        detail: false,
        raw: true,
        watch: false,
        once: false,
        base_url: None,
    })));
    assert!(should_emit_update_notice(&Commands::Current));
}

#[test]
fn update_check_cache_ttl_is_short_when_cached_version_matches_current() {
    assert_eq!(
        update_check_cache_ttl_seconds("0.2.47", "0.2.47"),
        UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS
    );
    assert_eq!(
        update_check_cache_ttl_seconds("0.2.46", "0.2.47"),
        UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS
    );
    assert_eq!(
        update_check_cache_ttl_seconds("0.2.48", "0.2.47"),
        UPDATE_CHECK_CACHE_TTL_SECONDS
    );
}

#[test]
fn format_info_prodex_version_reports_up_to_date_from_cache() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should be created");
    fs::write(
        update_check_cache_file_path(&paths),
        serde_json::to_string_pretty(&serde_json::json!({
            "source": "CratesIo",
            "latest_version": current_prodex_version(),
            "checked_at": Local::now().timestamp(),
        }))
        .expect("update cache json should serialize"),
    )
    .expect("update cache should save");

    assert_eq!(
        format_info_prodex_version(&paths).expect("version summary should render"),
        format!("{} (up to date)", current_prodex_version())
    );
}

#[test]
fn format_info_prodex_version_reports_available_update_from_cache() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should be created");
    fs::write(
        update_check_cache_file_path(&paths),
        serde_json::to_string_pretty(&serde_json::json!({
            "source": "CratesIo",
            "latest_version": "99.0.0",
            "checked_at": Local::now().timestamp(),
        }))
        .expect("update cache json should serialize"),
    )
    .expect("update cache should save");

    assert_eq!(
        format_info_prodex_version(&paths).expect("version summary should render"),
        format!("{} (update available: 99.0.0)", current_prodex_version())
    );
}

#[test]
fn normalize_run_codex_args_rewrites_session_id_to_resume() {
    let args = vec![
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("continue from here"),
    ];
    assert_eq!(
        normalize_run_codex_args(&args),
        vec![
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("continue from here"),
        ]
    );
}

#[test]
fn prepare_codex_launch_args_preserves_review_detection_after_normalization() {
    let (args, include_code_review) = prepare_codex_launch_args(&[
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("review"),
    ]);

    assert_eq!(
        args,
        vec![
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("review"),
        ]
    );
    assert!(include_code_review);
}

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
fn collect_info_runtime_load_summary_from_text_parses_recent_activity() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-03-30T12:10:00+07:00")
        .expect("timestamp should parse")
        .timestamp();
    let text = r#"
[2026-03-30 12:00:00.000 +07:00] profile_inflight profile=main count=2 weight=2 context=responses_http event=acquire
[2026-03-30 12:01:00.000 +07:00] selection_keep_current route=responses profile=main inflight=2 health=0 quota_source=probe_cache quota_band=quota_healthy five_hour_status=ready five_hour_remaining=80 five_hour_reset_at=1760000000 weekly_status=ready weekly_remaining=90 weekly_reset_at=1760500000
[2026-03-30 12:05:00.000 +07:00] selection_pick route=responses profile=main mode=ready inflight=1 health=0 order=0 quota_source=probe_cache quota_band=quota_healthy five_hour_status=ready five_hour_remaining=70 five_hour_reset_at=1760000000 weekly_status=ready weekly_remaining=85 weekly_reset_at=1760500000
[2026-03-30 12:06:00.000 +07:00] profile_inflight profile=main count=1 weight=1 context=standard_http event=release
"#;

    let summary = collect_info_runtime_load_summary_from_text(text, now, 30 * 60, 3 * 60 * 60);

    assert_eq!(summary.recent_selection_events, 2);
    assert_eq!(summary.active_inflight_units, 1);
    assert_eq!(summary.observations.len(), 2);
    assert_eq!(
        summary.recent_first_timestamp,
        Some(
            chrono::DateTime::parse_from_rfc3339("2026-03-30T12:01:00+07:00")
                .expect("timestamp should parse")
                .timestamp()
        )
    );
    assert_eq!(
        summary.recent_last_timestamp,
        Some(
            chrono::DateTime::parse_from_rfc3339("2026-03-30T12:05:00+07:00")
                .expect("timestamp should parse")
                .timestamp()
        )
    );
}

#[test]
fn estimate_info_runway_uses_latest_monotonic_segment_after_reset() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-03-30T11:00:00+07:00")
        .expect("timestamp should parse")
        .timestamp();
    let observations = vec![
        InfoRuntimeQuotaObservation {
            timestamp: now - 3_600,
            profile: "main".to_string(),
            five_hour_remaining: 20,
            weekly_remaining: 40,
        },
        InfoRuntimeQuotaObservation {
            timestamp: now - 1_800,
            profile: "main".to_string(),
            five_hour_remaining: 100,
            weekly_remaining: 80,
        },
        InfoRuntimeQuotaObservation {
            timestamp: now,
            profile: "main".to_string(),
            five_hour_remaining: 80,
            weekly_remaining: 70,
        },
        InfoRuntimeQuotaObservation {
            timestamp: now - 1_800,
            profile: "second".to_string(),
            five_hour_remaining: 90,
            weekly_remaining: 95,
        },
        InfoRuntimeQuotaObservation {
            timestamp: now,
            profile: "second".to_string(),
            five_hour_remaining: 60,
            weekly_remaining: 90,
        },
    ];

    let estimate = estimate_info_runway(&observations, InfoQuotaWindow::FiveHour, 140, now)
        .expect("five-hour runway should be estimated");

    assert_eq!(estimate.observed_profiles, 2);
    assert_eq!(estimate.observed_span_seconds, 1_800);
    assert!((estimate.burn_per_hour - 100.0).abs() < 0.001);
    assert_eq!(estimate.exhaust_at, now + 5_040);
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
            broker_key: runtime_broker_key(&backend.base_url(), false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
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
fn runtime_proxy_broker_metrics_endpoint_reports_live_runtime_snapshot() {
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
            broker_key: runtime_broker_key(&backend.base_url(), false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: None,
            executable_sha256: None,
        },
    );
    let mut log = std::fs::OpenOptions::new()
        .append(true)
        .open(&proxy.log_path)
        .expect("runtime log should open for append");
    writeln!(
        log,
        "[2026-04-22 10:00:00.000 +00:00] request=7 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main"
    )
    .expect("runtime log should append stale continuation");
    writeln!(
        log,
        "[2026-04-22 10:00:00.001 +00:00] request=7 transport=websocket route=websocket websocket_session=17 chain_dead_upstream_confirmed profile=main previous_response_id=resp-7 reason=previous_response_not_found_locked_affinity via=- event=-"
    )
    .expect("runtime log should append chain dead marker");
    drop(log);

    let response = Client::builder()
        .build()
        .expect("client")
        .get(format!(
            "http://{}/__prodex/runtime/metrics",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("runtime broker metrics request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let metrics = response
        .json::<RuntimeBrokerMetrics>()
        .expect("runtime broker metrics should decode");
    assert_eq!(metrics.health.current_profile, "main");
    assert_eq!(metrics.health.instance_token, "instance");
    assert_eq!(metrics.health.persistence_role, "owner");
    assert_eq!(
        metrics.health.prodex_version.as_deref(),
        Some(runtime_current_prodex_version())
    );
    assert!(metrics.active_request_limit > 0);
    assert!(metrics.traffic.responses.limit > 0);
    assert_eq!(metrics.traffic.responses.admissions_total, 0);
    assert_eq!(metrics.traffic.responses.global_limit_rejections_total, 0);
    assert_eq!(metrics.traffic.responses.lane_limit_rejections_total, 0);
    assert_eq!(metrics.local_overload_backoff_remaining_seconds, 0);
    assert_eq!(metrics.continuations.response_bindings, 0);
    assert_eq!(
        metrics.continuity_failure_reasons.stale_continuation,
        BTreeMap::from([("previous_response_not_found".to_string(), 1)])
    );
    assert_eq!(
        metrics
            .continuity_failure_reasons
            .chain_dead_upstream_confirmed,
        BTreeMap::from([(
            "previous_response_not_found_locked_affinity".to_string(),
            1,
        )])
    );
}

#[test]
fn runtime_proxy_broker_prometheus_metrics_endpoint_reports_text_snapshot() {
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
            broker_key: runtime_broker_key(&backend.base_url(), false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: None,
            executable_sha256: None,
        },
    );
    let mut log = std::fs::OpenOptions::new()
        .append(true)
        .open(&proxy.log_path)
        .expect("runtime log should open for append");
    writeln!(
        log,
        "[2026-04-22 10:00:00.010 +00:00] request=9 transport=websocket route=websocket websocket_session=21 chain_retried_owner profile=main previous_response_id=resp-9 delay_ms=20 reason=previous_response_not_found_locked_affinity via=-"
    )
    .expect("runtime log should append chain retry marker");
    writeln!(
        log,
        "[2026-04-22 10:00:00.011 +00:00] request=9 websocket_session=21 stale_continuation reason=websocket_reuse_watchdog_locked_affinity profile=main event=timeout"
    )
    .expect("runtime log should append stale continuation marker");
    drop(log);

    let response = Client::builder()
        .build()
        .expect("client")
        .get(format!(
            "http://{}/__prodex/runtime/metrics/prometheus",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("runtime broker prometheus request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/plain; version=0.0.4; charset=utf-8")
    );
    let body = response.text().expect("prometheus body should decode");
    assert!(body.contains("prodex_runtime_broker_info"));
    assert!(body.contains("broker_key=\""));
    assert!(body.contains("current_profile=\"main\""));
    assert!(body.contains("prodex_version=\""));
    assert!(body.contains("executable_sha256=\""));
    assert!(body.contains("prodex_runtime_broker_lane_admissions_total"));
    assert!(body.contains("prodex_runtime_broker_lane_global_limit_rejections_total"));
    assert!(body.contains("prodex_runtime_broker_lane_lane_limit_rejections_total"));
    assert!(body.contains("prodex_runtime_broker_continuation_binding_counts"));
    assert!(body.contains("prodex_runtime_broker_continuity_failures_total"));
    assert!(body.contains("binding_kind=\"response\""));
    assert!(body.contains("lifecycle=\"warm\""));
    assert!(body.contains(
        "event=\"chain_retried_owner\",listen_addr=\""
    ));
    assert!(body.contains(
        "reason=\"websocket_reuse_watchdog_locked_affinity\""
    ));
}

#[test]
fn runtime_broker_metrics_snapshot_tracks_lane_admissions_and_rejections() {
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
        },
        2,
    );

    let guard = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("responses admission should succeed");
    drop(guard);

    shared
        .active_request_count
        .store(shared.active_request_limit, Ordering::SeqCst);
    assert!(matches!(
        try_acquire_runtime_proxy_active_request_slot(
            &shared,
            "http",
            "/backend-api/codex/responses/compact",
        ),
        Err(RuntimeProxyAdmissionRejection::GlobalLimit)
    ));
    shared.active_request_count.store(0, Ordering::SeqCst);

    shared
        .lane_admission
        .responses_active
        .store(shared.lane_admission.limits.responses, Ordering::SeqCst);
    assert!(matches!(
        try_acquire_runtime_proxy_active_request_slot(
            &shared,
            "http",
            "/backend-api/codex/responses",
        ),
        Err(RuntimeProxyAdmissionRejection::LaneLimit(
            RuntimeRouteKind::Responses
        ))
    ));
    shared
        .lane_admission
        .responses_active
        .store(0, Ordering::SeqCst);

    let metrics = runtime_broker_metrics_snapshot(
        &shared,
        &RuntimeBrokerMetadata {
            broker_key: "broker".to_string(),
            listen_addr: "127.0.0.1:12345".to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
        },
    )
    .expect("broker metrics snapshot should succeed");

    assert_eq!(metrics.traffic.responses.admissions_total, 1);
    assert_eq!(metrics.traffic.responses.global_limit_rejections_total, 0);
    assert_eq!(metrics.traffic.responses.lane_limit_rejections_total, 1);
    assert_eq!(metrics.traffic.compact.admissions_total, 0);
    assert_eq!(metrics.traffic.compact.global_limit_rejections_total, 1);
    assert_eq!(metrics.traffic.compact.lane_limit_rejections_total, 0);
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
            broker_key: runtime_broker_key(&backend.base_url(), false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
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

include!("main_internal/runtime_proxy_continuations.rs");

#[test]
fn runtime_proxy_worker_count_env_override_beats_policy_file() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    fs::create_dir_all(&prodex_home).expect("prodex home should exist");
    fs::write(
        prodex_home.join("policy.toml"),
        r#"
version = 1

[runtime_proxy]
worker_count = 11
"#,
    )
    .expect("policy file should write");
    let _prodex_guard = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home.display().to_string());
    let _worker_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_WORKER_COUNT", "13");

    clear_runtime_policy_cache();
    assert_eq!(runtime_proxy_worker_count(), 13);
    clear_runtime_policy_cache();
}

#[test]
fn cleanup_runtime_broker_stale_leases_removes_dead_pid_files() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "lease-test";
    let lease_dir = runtime_broker_lease_dir(&paths, broker_key);
    fs::create_dir_all(&lease_dir).expect("lease dir should exist");
    let stale_path = lease_dir.join("999999999-stale.lease");
    let live_path = lease_dir.join(format!("{}-live.lease", std::process::id()));
    fs::write(&stale_path, "stale").expect("stale lease should write");
    fs::write(&live_path, "live").expect("live lease should write");

    let live_count = cleanup_runtime_broker_stale_leases(&paths, broker_key);

    assert_eq!(live_count, 1);
    assert!(!stale_path.exists(), "dead-pid lease should be removed");
    assert!(live_path.exists(), "current-pid lease should remain");
}

#[test]
fn runtime_proxy_endpoint_child_lease_uses_requested_pid_and_cleans_up() {
    let temp_dir = TestDir::new();
    let lease_dir = temp_dir.path.join("leases");
    let endpoint = RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:33475".parse().expect("listen addr should parse"),
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        lease_dir: lease_dir.clone(),
        _lease: None,
    };

    let child_pid = 424242u32;
    let lease = endpoint
        .create_child_lease(child_pid)
        .expect("child lease should be created");
    let mut entries = fs::read_dir(&lease_dir)
        .expect("lease dir should exist")
        .collect::<Result<Vec<_>, _>>()
        .expect("lease dir should be readable");
    assert_eq!(entries.len(), 1, "expected exactly one child lease file");
    let lease_path = entries
        .pop()
        .expect("child lease entry should exist")
        .path();
    let file_name = lease_path
        .file_name()
        .and_then(|value| value.to_str())
        .expect("lease file name should be utf-8")
        .to_string();
    assert!(
        file_name.starts_with("424242-"),
        "child lease should use the child pid in its filename: {file_name}"
    );
    assert_eq!(
        fs::read_to_string(&lease_path).expect("lease file should be readable"),
        "pid=424242\n"
    );

    drop(lease);
    let remaining = fs::read_dir(&lease_dir)
        .expect("lease dir should still exist")
        .count();
    assert_eq!(remaining, 0, "dropping the lease should remove the file");
}

#[test]
fn runtime_broker_process_args_only_include_review_flag_when_enabled() {
    let without_review = runtime_broker_process_args(
        "main",
        "https://chatgpt.com/backend-api",
        false,
        "broker-key",
        "instance",
        "admin",
        None,
    );
    let without_review: Vec<String> = without_review
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    assert!(
        !without_review
            .iter()
            .any(|value| value == "--include-code-review"),
        "false should not emit a stray boolean value for the review flag"
    );

    let with_review = runtime_broker_process_args(
        "main",
        "https://chatgpt.com/backend-api",
        true,
        "broker-key",
        "instance",
        "admin",
        Some("127.0.0.1:33475"),
    );
    let with_review: Vec<String> = with_review
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    assert!(
        with_review
            .iter()
            .any(|value| value == "--include-code-review"),
        "true should emit the review flag"
    );
    assert!(
        !with_review
            .iter()
            .any(|value| value == "true" || value == "false"),
        "review flag must be encoded as a clap boolean switch"
    );
    assert!(
        with_review
            .windows(2)
            .any(|pair| pair == ["--listen-addr", "127.0.0.1:33475"]),
        "listen addr should be forwarded when requested"
    );
}

#[test]
fn runtime_broker_key_is_scoped_to_prodex_binary_identity() {
    let first = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        false,
        "version=0.39.0;sha256=alpha",
    );
    let second = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        false,
        "version=0.39.0;sha256=beta",
    );
    let third = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        false,
        "version=0.40.0;sha256=alpha",
    );
    let review = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        true,
        "version=0.39.0;sha256=alpha",
    );

    assert_ne!(
        first, second,
        "runtime broker keys must not reuse brokers from a different binary build"
    );
    assert_ne!(
        first, third,
        "runtime broker keys must not reuse brokers from a different prodex version"
    );
    assert_ne!(
        first, review,
        "review and non-review broker routes should stay isolated"
    );
}

#[test]
fn preferred_runtime_broker_listen_addr_only_reuses_dead_registry_ports() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "reuse-port-test";
    save_runtime_broker_registry(
        &paths,
        broker_key,
        &RuntimeBrokerRegistry {
            pid: 999_999_999,
            listen_addr: "127.0.0.1:33475".to_string(),
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
        },
    )
    .expect("dead broker registry should save");

    assert_eq!(
        preferred_runtime_broker_listen_addr(&paths, broker_key)
            .expect("dead broker port lookup should succeed"),
        Some("127.0.0.1:33475".to_string())
    );

    save_runtime_broker_registry(
        &paths,
        broker_key,
        &RuntimeBrokerRegistry {
            pid: std::process::id(),
            listen_addr: "127.0.0.1:33475".to_string(),
            started_at: Local::now().timestamp(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            instance_token: "live-instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        },
    )
    .expect("live broker registry should save");

    assert_eq!(
        preferred_runtime_broker_listen_addr(&paths, broker_key)
            .expect("live broker port lookup should succeed"),
        None
    );
}

#[test]
fn runtime_rotation_proxy_can_bind_a_requested_listen_addr() {
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
    let probe = TcpListener::bind("127.0.0.1:0").expect("probe socket should bind");
    let requested_addr = probe
        .local_addr()
        .expect("probe socket should expose requested addr");
    drop(probe);

    let proxy = start_runtime_rotation_proxy_with_listen_addr(
        &paths,
        &state,
        "main",
        backend.base_url(),
        false,
        Some(&requested_addr.to_string()),
    )
    .expect("runtime proxy should bind requested listen addr");

    assert_eq!(proxy.listen_addr, requested_addr);
}

#[test]
fn runtime_broker_lease_drop_removes_file() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let lease = create_runtime_broker_lease(&paths, "drop-test")
        .expect("lease should be created for drop test");
    let lease_path = lease.path.clone();
    assert!(lease_path.exists(), "lease file should exist before drop");

    drop(lease);

    assert!(
        !lease_path.exists(),
        "lease file should be removed when the endpoint drops it"
    );
}

include!("main_internal/runtime_broker_registry.rs");

#[test]
fn runtime_broker_startup_grace_covers_ready_timeout() {
    let _timeout_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "15000");
    assert!(runtime_broker_startup_grace_seconds() >= 16);
}

#[test]
fn runtime_broker_command_is_the_only_command_without_update_notice() {
    let runtime_broker = Commands::RuntimeBroker(RuntimeBrokerArgs {
        current_profile: "main".to_string(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        broker_key: "broker".to_string(),
        instance_token: "instance".to_string(),
        admin_token: "admin".to_string(),
        listen_addr: None,
    });
    let run = Commands::Run(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        skip_quota_check: false,
        base_url: None,
        codex_args: vec![OsString::from("hello")],
    });

    assert!(!runtime_broker.should_show_update_notice());
    assert!(run.should_show_update_notice());
}

#[test]
fn claude_command_accepts_passthrough_args() {
    let command = parse_cli_command_from([
        "prodex",
        "claude",
        "--profile",
        "main",
        "--",
        "-p",
        "--output-format",
        "json",
        "hello",
    ])
    .expect("claude command should parse");
    let Commands::Claude(args) = command else {
        panic!("expected claude command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.claude_args,
        vec![
            OsString::from("-p"),
            OsString::from("--output-format"),
            OsString::from("json"),
            OsString::from("hello"),
        ]
    );
}

#[test]
fn claude_caveman_mode_extracts_prefix_and_preserves_passthrough_args() {
    let (launch_modes, claude_args) = runtime_proxy_claude_extract_launch_modes(&[
        OsString::from("caveman"),
        OsString::from("-p"),
        OsString::from("hello"),
    ]);
    assert!(launch_modes.caveman_mode);
    assert!(!launch_modes.mem_mode);
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );

    let (launch_modes, claude_args) =
        runtime_proxy_claude_extract_launch_modes(&[OsString::from("-p"), OsString::from("hi")]);
    assert!(!launch_modes.caveman_mode);
    assert!(!launch_modes.mem_mode);
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hi")]
    );
}

#[test]
fn runtime_proxy_claude_launch_modes_extract_mem_and_caveman_prefixes() {
    let (launch_modes, claude_args) = runtime_proxy_claude_extract_launch_modes(&[
        OsString::from("caveman"),
        OsString::from("mem"),
        OsString::from("-p"),
        OsString::from("hello"),
    ]);
    assert_eq!(
        launch_modes,
        RuntimeProxyClaudeLaunchModes {
            caveman_mode: true,
            mem_mode: true,
        }
    );
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );

    let (launch_modes, claude_args) = runtime_proxy_claude_extract_launch_modes(&[
        OsString::from("mem"),
        OsString::from("caveman"),
        OsString::from("--print"),
    ]);
    assert_eq!(
        launch_modes,
        RuntimeProxyClaudeLaunchModes {
            caveman_mode: true,
            mem_mode: true,
        }
    );
    assert_eq!(claude_args, vec![OsString::from("--print")]);
}

#[test]
fn runtime_mem_extract_mode_strips_only_leading_mem_prefix() {
    let (mem_mode, codex_args) =
        runtime_mem_extract_mode(&[OsString::from("mem"), OsString::from("exec")]);
    assert!(mem_mode);
    assert_eq!(codex_args, vec![OsString::from("exec")]);

    let (mem_mode, codex_args) =
        runtime_mem_extract_mode(&[OsString::from("exec"), OsString::from("mem")]);
    assert!(!mem_mode);
    assert_eq!(
        codex_args,
        vec![OsString::from("exec"), OsString::from("mem")]
    );
}

#[test]
fn runtime_proxy_claude_launch_args_prepend_plugin_dirs_when_present() {
    let launch_args = runtime_proxy_claude_launch_args(
        &[OsString::from("-p"), OsString::from("hello")],
        &[
            PathBuf::from("/tmp/claude-mem-plugin"),
            PathBuf::from("/tmp/prodex-caveman-plugin"),
        ],
    );
    assert_eq!(
        launch_args,
        vec![
            OsString::from("--plugin-dir"),
            OsString::from("/tmp/claude-mem-plugin"),
            OsString::from("--plugin-dir"),
            OsString::from("/tmp/prodex-caveman-plugin"),
            OsString::from("-p"),
            OsString::from("hello"),
        ]
    );

    let launch_args =
        runtime_proxy_claude_launch_args(&[OsString::from("-p"), OsString::from("hello")], &[]);
    assert_eq!(
        launch_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );
}

#[test]
fn prepare_runtime_proxy_claude_caveman_plugin_dir_installs_local_plugin_bundle() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.clone(),
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("profiles"),
        shared_codex_root: temp_dir.path.join(".codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
    };

    let plugin_dir = prepare_runtime_proxy_claude_caveman_plugin_dir(&paths)
        .expect("Claude Caveman plugin dir should prepare");
    assert!(
        plugin_dir.join(".claude-plugin/plugin.json").is_file(),
        "plugin manifest should exist"
    );
    assert!(
        plugin_dir.join("commands/caveman.toml").is_file(),
        "caveman command should exist"
    );
    assert!(
        plugin_dir.join("skills/caveman/SKILL.md").is_file(),
        "caveman skill should exist"
    );

    let activate_hook = fs::read_to_string(plugin_dir.join("hooks/caveman-activate.js"))
        .expect("activation hook should read");
    assert!(activate_hook.contains("CLAUDE_CONFIG_DIR"));
    let tracker_hook = fs::read_to_string(plugin_dir.join("hooks/caveman-mode-tracker.js"))
        .expect("tracker hook should read");
    assert!(tracker_hook.contains("getClaudeConfigDir"));
    let statusline = fs::read_to_string(plugin_dir.join("hooks/caveman-statusline.sh"))
        .expect("statusline script should read");
    assert!(statusline.contains("CLAUDE_CONFIG_DIR"));
}

#[test]
fn runtime_mem_claude_plugin_dir_from_home_uses_marketplace_install_path() {
    let temp_dir = TestDir::new();
    let home = temp_dir.path.join("home");
    assert_eq!(
        runtime_mem_claude_plugin_dir_from_home(&home),
        home.join(".claude")
            .join("plugins")
            .join("marketplaces")
            .join("thedotmack")
            .join("plugin")
    );
}

#[test]
fn runtime_mem_transcript_watch_config_path_from_home_prefers_settings_override() {
    let temp_dir = TestDir::new();
    let home = temp_dir.path.join("home");
    let data_dir = runtime_mem_data_dir_from_home(&home);
    fs::create_dir_all(&data_dir).expect("claude-mem data dir should exist");
    fs::write(
        data_dir.join("settings.json"),
        serde_json::json!({
            "CLAUDE_MEM_TRANSCRIPTS_CONFIG_PATH": data_dir.join("custom-watch.json").display().to_string()
        })
        .to_string(),
    )
    .expect("settings should write");

    assert_eq!(
        runtime_mem_transcript_watch_config_path_from_home(&home),
        data_dir.join("custom-watch.json")
    );
}

#[test]
fn ensure_runtime_mem_prodex_observer_writes_wrapper_and_settings() {
    let temp_dir = TestDir::new();
    let home = temp_dir.path.join("home");
    let settings_path = runtime_mem_settings_path_from_home(&home);
    fs::create_dir_all(settings_path.parent().expect("settings parent"))
        .expect("settings parent should exist");
    fs::write(
        &settings_path,
        serde_json::json!({
            "CLAUDE_MEM_PROVIDER": "claude",
            "CLAUDE_CODE_PATH": "/usr/bin/claude"
        })
        .to_string(),
    )
    .expect("settings should write");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex-home"),
        state_file: temp_dir.path.join("prodex-home/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex-home/profiles"),
        shared_codex_root: temp_dir.path.join("prodex-home/.codex"),
        legacy_shared_codex_root: temp_dir.path.join("prodex-home/shared"),
    };
    let prodex_exe = temp_dir.path.join("bin/prodex");
    fs::create_dir_all(prodex_exe.parent().expect("prodex bin parent"))
        .expect("prodex bin parent should exist");
    fs::write(&prodex_exe, "").expect("prodex exe should write");

    let wrapper_path = ensure_runtime_mem_prodex_observer_for_home(&home, &paths, &prodex_exe)
        .expect("prodex observer should configure");

    assert_eq!(wrapper_path, runtime_mem_prodex_claude_wrapper_path(&paths));
    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings_path).expect("settings should read"))
            .expect("settings should parse");
    assert_eq!(settings["CLAUDE_MEM_PROVIDER"], serde_json::json!("claude"));
    assert_eq!(
        settings["CLAUDE_CODE_PATH"],
        serde_json::json!(wrapper_path.display().to_string())
    );

    let wrapper = fs::read_to_string(&wrapper_path).expect("wrapper should read");
    assert!(wrapper.contains(" claude --skip-quota-check -- "));
    assert!(wrapper.contains(&prodex_exe.display().to_string()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&wrapper_path)
                .expect("wrapper metadata should read")
                .permissions()
                .mode()
                & 0o111,
            0o111,
            "wrapper should be executable"
        );
    }
}

#[test]
fn ensure_runtime_mem_codex_watch_for_home_adds_prodex_watch_without_clobbering_default_watch() {
    let temp_dir = TestDir::new();
    let config_path = temp_dir.path.join("claude-mem/transcript-watch.json");
    fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("config parent should exist");
    fs::write(
        &config_path,
        serde_json::json!({
            "version": 1,
            "schemas": {
                "codex": runtime_mem_default_codex_schema(),
            },
            "watches": [{
                "name": "codex",
                "path": "~/.codex/sessions/**/*.jsonl",
                "schema": "codex",
                "startAtEnd": true,
                "context": {
                    "mode": "agents",
                    "updateOn": ["session_start", "session_end"],
                }
            }]
        })
        .to_string(),
    )
    .expect("transcript watch config should write");

    let sessions_root = temp_dir.path.join("prodex-shared/sessions");
    fs::create_dir_all(&sessions_root).expect("sessions root should exist");
    let codex_home = temp_dir.path.join("codex-home");
    fs::create_dir_all(&codex_home).expect("codex home should exist");
    runtime_proxy_create_symlink(&sessions_root, &codex_home.join("sessions"), true)
        .expect("sessions symlink should create");

    ensure_runtime_mem_codex_watch_for_home_at_path(&config_path, &codex_home)
        .expect("prodex codex watch should be added");
    ensure_runtime_mem_codex_watch_for_home_at_path(&config_path, &codex_home)
        .expect("prodex codex watch should stay deduplicated");

    let rendered: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&config_path).expect("transcript watch config should read"),
    )
    .expect("transcript watch config should parse");
    let watches = rendered["watches"]
        .as_array()
        .expect("watches should be an array");
    assert_eq!(watches.len(), 2);
    assert_eq!(watches[0]["name"], serde_json::json!("codex"));
    let prodex_watch = watches
        .iter()
        .find(|watch| {
            watch["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("prodex-codex-"))
        })
        .expect("prodex watch should exist");
    assert_eq!(
        prodex_watch["path"],
        serde_json::json!(format!(
            "{}{}**{}*.jsonl",
            sessions_root.display(),
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR
        ))
    );
}

#[test]
fn prepare_caveman_launch_home_localizes_config_and_installs_plugin() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.clone(),
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("profiles"),
        shared_codex_root: temp_dir.path.join(".codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
    };
    create_codex_home_if_missing(&paths.shared_codex_root).expect("shared codex root");
    create_codex_home_if_missing(&paths.managed_profiles_root).expect("managed root");
    let shared_config = paths.shared_codex_root.join("config.toml");
    fs::write(
        &shared_config,
        "model = \"gpt-5\"\n[features]\nsearch_tool = true\n",
    )
    .expect("shared config should write");

    let base_home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&base_home).expect("base home");
    runtime_proxy_create_symlink(&shared_config, &base_home.join("config.toml"), false)
        .expect("config symlink should create");
    fs::write(base_home.join("auth.json"), "{}").expect("auth file should write");

    let caveman_home =
        prepare_caveman_launch_home(&paths, &base_home).expect("caveman home should prepare");
    let temp_config = caveman_home.join("config.toml");
    let metadata = fs::symlink_metadata(&temp_config).expect("temp config metadata");
    assert!(
        !metadata.file_type().is_symlink(),
        "temporary Caveman config should be detached from the shared config symlink"
    );

    let rendered_config = fs::read_to_string(&temp_config).expect("temp config should read");
    assert!(rendered_config.contains("plugins = true"));
    assert!(rendered_config.contains("codex_hooks = true"));
    assert!(rendered_config.contains("suppress_unstable_features_warning = true"));
    assert!(rendered_config.contains("[marketplaces.prodex-caveman]"));
    assert!(rendered_config.contains("[plugins.\"caveman@prodex-caveman\"]"));
    assert!(rendered_config.contains("enabled = true"));

    let shared_rendered = fs::read_to_string(&shared_config).expect("shared config should read");
    assert!(
        !shared_rendered.contains("prodex-caveman"),
        "base shared config must stay unchanged"
    );
    assert!(
        !base_home.join("hooks.json").exists(),
        "base home should not gain a persistent hooks.json file"
    );

    let hooks: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(caveman_home.join("hooks.json")).expect("hooks should read"),
    )
    .expect("hooks should parse");
    assert_eq!(
        hooks["hooks"]["SessionStart"][0]["hooks"][0]["type"],
        serde_json::Value::String("command".to_string())
    );

    let marketplace_path =
        caveman_home.join(".tmp/marketplaces/prodex-caveman/.agents/plugins/marketplace.json");
    let marketplace_text =
        fs::read_to_string(&marketplace_path).expect("marketplace manifest should read");
    assert!(marketplace_text.contains("\"name\": \"prodex-caveman\""));
    assert!(
        caveman_home
            .join(".tmp/marketplaces/prodex-caveman/plugins/caveman/.codex-plugin/plugin.json")
            .is_file()
    );
    assert!(
        caveman_home
            .join("plugins/cache/prodex-caveman/caveman/0.1.0/.codex-plugin/plugin.json")
            .is_file()
    );
}


include!("main_internal/runtime_proxy_claude_and_anthropic.rs");
