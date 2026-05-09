use super::*;
use crate::TestEnvVarGuard;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[path = "main_internal/runtime_proxy_backend.rs"]
mod runtime_proxy_backend;
#[path = "main_internal/runtime_test_auth.rs"]
mod runtime_test_auth;
#[path = "main_internal/runtime_test_support.rs"]
mod runtime_test_support;
#[path = "main_internal/runtime_test_websocket.rs"]
mod runtime_test_websocket;

use runtime_proxy_backend::*;
use runtime_test_auth::*;
use runtime_test_support::*;
use runtime_test_websocket::*;

#[path = "main_internal/runtime_proxy_continuation_helpers.rs"]
mod runtime_proxy_continuation_helpers;
use runtime_proxy_continuation_helpers::*;

#[path = "main_internal_body/context_commands.rs"]
mod context_commands;

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

#[path = "main_internal_body/startup_probe.rs"]
mod startup_probe;

#[path = "main_internal/runtime_proxy_selection_and_pressure.rs"]
mod runtime_proxy_selection_and_pressure;

#[path = "main_internal_body/update_doctor_launch.rs"]
mod update_doctor_launch;

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
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1,)])
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
    assert!(body.contains("prodex_runtime_broker_lane_releases_total"));
    assert!(body.contains("prodex_runtime_broker_lane_global_limit_rejections_total"));
    assert!(body.contains("prodex_runtime_broker_lane_lane_limit_rejections_total"));
    assert!(body.contains(
        "prodex_runtime_broker_lane_release_underflows_total"
    ));
    assert!(body.contains(
        "prodex_runtime_broker_active_request_release_underflows_total"
    ));
    assert!(body.contains(
        "prodex_runtime_broker_profile_inflight_release_underflows_total"
    ));
    assert!(body.contains("prodex_runtime_broker_continuation_binding_counts"));
    assert!(body.contains("prodex_runtime_broker_continuity_failures_total"));
    assert!(body.contains("binding_kind=\"response\""));
    assert!(body.contains("lifecycle=\"warm\""));
    assert!(body.contains("event=\"chain_retried_owner\",listen_addr=\""));
    assert!(body.contains("reason=\"websocket_reuse_watchdog_locked_affinity\""));
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
            upstream_no_proxy: false,
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
        },
    )
    .expect("broker metrics snapshot should succeed");

    assert_eq!(metrics.traffic.responses.admissions_total, 1);
    assert_eq!(metrics.traffic.responses.releases_total, 1);
    assert_eq!(metrics.traffic.responses.release_underflows_total, 0);
    assert_eq!(metrics.traffic.responses.global_limit_rejections_total, 0);
    assert_eq!(metrics.traffic.responses.lane_limit_rejections_total, 1);
    assert_eq!(metrics.traffic.compact.admissions_total, 0);
    assert_eq!(metrics.traffic.compact.releases_total, 0);
    assert_eq!(metrics.traffic.compact.global_limit_rejections_total, 1);
    assert_eq!(metrics.traffic.compact.lane_limit_rejections_total, 0);
    assert_eq!(metrics.active_request_release_underflows_total, 0);
    assert_eq!(metrics.profile_inflight_admissions_total, 0);
    assert_eq!(metrics.profile_inflight_releases_total, 0);
    assert_eq!(metrics.profile_inflight_release_underflows_total, 0);
}

#[test]
fn runtime_broker_metrics_snapshot_records_runtime_state_lock_wait() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let runtime = RuntimeRotationState {
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
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime.clone(), 2);
    let isolated_shared = runtime_rotation_proxy_shared(&temp_dir, runtime, 2);
    shared.reset_runtime_state_lock_wait_metrics_for_test();
    isolated_shared.reset_runtime_state_lock_wait_metrics_for_test();

    let runtime_guard = shared.runtime.lock().expect("runtime lock should succeed");
    let worker_shared = shared.clone();
    let (started_sender, started_receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        started_sender.send(()).expect("snapshot start should send");
        runtime_broker_metrics_snapshot(
            &worker_shared,
            &RuntimeBrokerMetadata {
                broker_key: "broker".to_string(),
                listen_addr: "127.0.0.1:12345".to_string(),
                started_at: Local::now().timestamp(),
                current_profile: "main".to_string(),
                include_code_review: false,
                upstream_no_proxy: false,
                instance_token: "instance".to_string(),
                admin_token: "secret".to_string(),
                prodex_version: None,
                executable_path: None,
                executable_sha256: None,
            },
        )
        .expect("broker metrics snapshot should succeed");
    });

    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("snapshot thread should start");
    thread::sleep(Duration::from_millis(25));
    drop(runtime_guard);
    worker.join().expect("snapshot thread should join");

    let metrics = shared.runtime_state_lock_wait_metrics();
    assert_eq!(metrics.wait_count, 1);
    assert!(
        metrics.wait_total_ns >= 1_000_000,
        "lock wait total should include contention, got {metrics:?}"
    );
    assert!(
        metrics.wait_max_ns >= 1_000_000,
        "lock wait max should include contention, got {metrics:?}"
    );
    assert_eq!(
        isolated_shared.runtime_state_lock_wait_metrics(),
        RuntimeStateLockWaitMetrics::default()
    );
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

#[path = "main_internal/runtime_proxy_continuations.rs"]
mod runtime_proxy_continuations;

#[path = "main_internal_body/runtime_broker_tuning.rs"]
mod runtime_broker_tuning;

#[test]
fn runtime_smart_context_proxy_rewrites_large_tool_output_and_logs_budget() {
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
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
    let proxy = start_runtime_rotation_proxy_with_options(
        &paths,
        &state,
        "second",
        backend.base_url(),
        false,
        false,
        true,
        None,
        None,
    )
    .expect("runtime proxy should start with smart context enabled");
    let tool_output = (0..1900)
        .map(|index| format!("line {index}: repeated command output"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_big",
            "output": tool_output,
        }]
    })
    .to_string();
    let estimated_tokens =
        runtime_proxy_crate::smart_context_estimate_tokens_from_body(body.as_bytes()) as usize;
    let available_tokens = 32_000usize
        .saturating_sub(estimated_tokens)
        .saturating_sub(4_096);
    assert_eq!(
        runtime_proxy_crate::smart_context_token_budget_tier(available_tokens),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed
    );

    let response = Client::builder()
        .timeout(ci_timing_upper_bound_ms(5_000, 10_000))
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .expect("responses request should succeed");

    assert!(
        response.status().is_success(),
        "smart-context request should pass through upstream status: {}",
        response.status()
    );
    let responses_bodies = backend.responses_bodies();
    assert_eq!(responses_bodies.len(), 1);
    assert!(responses_bodies[0].contains("psc art psc:"));
    assert!(!responses_bodies[0].contains("prodex-artifact:sc:"));
    assert!(
        !responses_bodies[0].contains("line 1200: repeated command output"),
        "middle tool-output noise should be artifact-backed, not forwarded inline"
    );

    let log_tail = wait_for_runtime_log_tail_until(
        || fs::read(&proxy.log_path).ok(),
        |text| text.contains("smart_context_autopilot") && text.contains("decision=rewritten"),
        2_000,
        8_000,
        20,
    );
    let log_tail = String::from_utf8_lossy(&log_tail);
    assert!(log_tail.contains("tier=condensed"));
    assert!(log_tail.contains("budget_mode=artifact_condensed"));
    assert!(log_tail.contains("policy_reasons=tight_budget"));
    assert!(log_tail.contains("artifacts_stored=1"));
    assert!(log_tail.contains("tool_outputs_condensed=1"));
}

#[test]
fn runtime_smart_context_proxy_disabled_passes_large_tool_output_unchanged() {
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
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
    let proxy = start_runtime_rotation_proxy(
        &paths,
        &state,
        "second",
        backend.base_url(),
        false,
    )
    .expect("runtime proxy should start with smart context disabled");
    let tool_output = (0..2500)
        .map(|index| format!("line {index}: repeated command output"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_big",
            "output": tool_output,
        }]
    })
    .to_string();

    let response = Client::builder()
        .timeout(ci_timing_upper_bound_ms(2_000, 8_000))
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .expect("responses request should succeed");

    assert!(
        response.status().is_success(),
        "disabled smart-context request should pass through upstream status: {}",
        response.status()
    );
    let responses_bodies = backend.responses_bodies();
    assert_eq!(responses_bodies.len(), 1);
    assert!(!responses_bodies[0].contains("prodex-sc artifact"));
    assert!(responses_bodies[0].contains("line 1200: repeated command output"));
    let log = fs::read_to_string(&proxy.log_path).expect("runtime proxy log should be readable");
    assert!(!log.contains("smart_context_autopilot"));
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
            upstream_no_proxy: false,
            smart_context_enabled: false,
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
            upstream_no_proxy: false,
            smart_context_enabled: false,
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

#[path = "main_internal/runtime_broker_registry.rs"]
mod runtime_broker_registry;

#[test]
fn runtime_broker_startup_grace_covers_ready_timeout() {
    let _timeout_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "15000");
    assert!(runtime_broker_startup_grace_seconds() >= 16);
}

#[test]
fn runtime_broker_and_update_commands_skip_prodex_update_notice() {
    let runtime_broker = Commands::RuntimeBroker(RuntimeBrokerArgs {
        current_profile: "main".to_string(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        upstream_no_proxy: false,
        smart_context_enabled: false,
        model_context_window_tokens: None,
        broker_key: "broker".to_string(),
        instance_token: "instance".to_string(),
        admin_token: "admin".to_string(),
        listen_addr: None,
    });
    let update = Commands::Update(CodexUpdateArgs {
        codex_args: vec![OsString::from("--check")],
    });
    let run = Commands::Run(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_args: vec![OsString::from("hello")],
    });

    assert!(!runtime_broker.should_show_update_notice());
    assert!(!update.should_show_update_notice());
    assert!(run.should_show_update_notice());
}

#[test]
fn update_command_accepts_passthrough_args() {
    let command = parse_cli_command_from([
        "prodex",
        "update",
        "--check",
        "rust-v0.128.0",
        "--force",
    ])
    .expect("update command should parse");
    let Commands::Update(args) = command else {
        panic!("expected update command");
    };
    assert_eq!(
        args.codex_args,
        vec![
            OsString::from("--check"),
            OsString::from("rust-v0.128.0"),
            OsString::from("--force"),
        ]
    );

    let command =
        parse_cli_command_from(["prodex", "update", "--help"]).expect("update help should pass");
    let Commands::Update(args) = command else {
        panic!("expected update command");
    };
    assert_eq!(args.codex_args, vec![OsString::from("--help")]);
}

#[test]
fn launch_commands_accept_dry_run_as_prodex_flag() {
    let run = parse_cli_command_from(["prodex", "run", "--dry-run", "exec", "hello"])
        .expect("run dry-run should parse");
    let Commands::Run(run_args) = run else {
        panic!("expected run command");
    };
    assert!(run_args.dry_run);
    assert_eq!(
        run_args.codex_args,
        vec![OsString::from("exec"), OsString::from("hello")]
    );

    let caveman = parse_cli_command_from(["prodex", "caveman", "--dry-run", "exec", "hello"])
        .expect("caveman dry-run should parse");
    let Commands::Caveman(caveman_args) = caveman else {
        panic!("expected caveman command");
    };
    assert!(caveman_args.dry_run);
    assert_eq!(
        caveman_args.codex_args,
        vec![OsString::from("exec"), OsString::from("hello")]
    );

    let super_command = parse_cli_command_from(["prodex", "super", "--dry-run", "exec", "hello"])
        .expect("super dry-run should parse");
    let Commands::Super(super_args) = super_command else {
        panic!("expected super command");
    };
    assert!(super_args.dry_run);
    assert_eq!(
        super_args.codex_args,
        vec![OsString::from("exec"), OsString::from("hello")]
    );
}

#[path = "main_internal_body/claude_launch.rs"]
mod claude_launch;

#[path = "main_internal/runtime_proxy_claude_and_anthropic.rs"]
mod runtime_proxy_claude_and_anthropic;
