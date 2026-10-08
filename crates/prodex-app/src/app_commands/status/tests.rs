use super::*;
use std::collections::HashSet;

fn status_usage_window(
    used_percent: Option<i64>,
    reset_at: Option<i64>,
    limit_window_seconds: i64,
) -> crate::UsageWindow {
    crate::UsageWindow {
        used_percent,
        reset_at,
        limit_window_seconds: Some(limit_window_seconds),
    }
}

fn status_usage(
    five_hour: Option<crate::UsageWindow>,
    weekly: Option<crate::UsageWindow>,
) -> prodex_quota::UsageResponse {
    prodex_quota::UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(crate::WindowPair {
            allowed: None,
            limit_reached: None,
            extra: BTreeMap::new(),
            primary_window: five_hour,
            secondary_window: weekly,
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    }
}

fn status_report(
    name: &str,
    order_index: usize,
    quota_compatible: bool,
    result: std::result::Result<prodex_quota::UsageResponse, String>,
) -> crate::RunProfileProbeReport {
    crate::RunProfileProbeReport {
        name: name.to_string(),
        order_index,
        auth: crate::AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible,
        },
        result,
    }
}

fn status_usage_snapshot(
    checked_at: i64,
    five_hour_status: prodex_runtime_state::RuntimeQuotaWindowStatus,
    five_hour_remaining_percent: i64,
    five_hour_reset_at: i64,
    weekly_status: prodex_runtime_state::RuntimeQuotaWindowStatus,
    weekly_remaining_percent: i64,
    weekly_reset_at: i64,
) -> crate::RuntimeProfileUsageSnapshot {
    crate::RuntimeProfileUsageSnapshot {
        checked_at,
        plan_type: None,
        five_hour_status,
        five_hour_remaining_percent,
        five_hour_reset_at,
        weekly_status,
        weekly_remaining_percent,
        weekly_reset_at,
    }
}

fn status_quota_fixture(
    now: i64,
) -> (
    Vec<crate::RunProfileProbeReport>,
    BTreeMap<String, crate::RuntimeProfileUsageSnapshot>,
) {
    use prodex_runtime_state::RuntimeQuotaWindowStatus::{Ready, Unknown};

    let reports = vec![
        status_report(
            "fresh",
            0,
            true,
            Ok(status_usage(
                Some(status_usage_window(Some(25), Some(1_200), 18_000)),
                Some(status_usage_window(Some(40), Some(2_000), 604_800)),
            )),
        ),
        status_report(
            "日本語 🐴",
            1,
            true,
            Ok(status_usage(
                None,
                Some(status_usage_window(Some(90), Some(1_800), 604_800)),
            )),
        ),
        status_report(
            "malformed",
            2,
            true,
            Ok(status_usage(
                Some(status_usage_window(None, Some(1_400), 18_000)),
                None,
            )),
        ),
        status_report("cached", 3, true, Err("probe failed".to_string())),
        status_report("stale", 4, true, Err("probe failed".to_string())),
        status_report("incompatible", 5, false, Err("probe failed".to_string())),
        status_report("success-empty", 6, true, Ok(status_usage(None, None))),
        status_report("no-cache", 7, true, Err("probe failed".to_string())),
    ];
    let snapshots = BTreeMap::from([
        (
            "cached".to_string(),
            status_usage_snapshot(now - 100, Ready, 55, 2_500, Unknown, 0, i64::MAX),
        ),
        (
            "stale".to_string(),
            status_usage_snapshot(
                now - RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS - 1,
                Ready,
                75,
                1_100,
                Unknown,
                0,
                i64::MAX,
            ),
        ),
        (
            "incompatible".to_string(),
            status_usage_snapshot(now - 100, Ready, 80, 1_100, Unknown, 0, i64::MAX),
        ),
        (
            "success-empty".to_string(),
            status_usage_snapshot(now - 100, Ready, 80, 1_100, Unknown, 0, i64::MAX),
        ),
    ]);
    (reports, snapshots)
}

#[test]
fn proc_parsers_extract_cpu_memory_and_disk_counters() {
    assert_eq!(
        parse_process_cpu_ticks("123 (prodex worker) S 1 2 3 4 5 6 7 8 9 10 120 30 0 0 0"),
        Some(150)
    );
    assert_eq!(
        parse_system_cpu_ticks("cpu  10 20 30 40 50 60 70 80 90\ncpu0 1 2 3 4"),
        Some(360)
    );
    assert_eq!(
        parse_kib_field("VmRSS: 2048 kB\n", "VmRSS"),
        Some(2_097_152)
    );
    assert_eq!(
        parse_u64_field("read_bytes: 123\nwrite_bytes: 456\n", "write_bytes"),
        Some(456)
    );
}

#[test]
fn resource_snapshot_derives_cpu_and_disk_rates() {
    let previous = StatusResourceCounters {
        available: true,
        process_cpu_ticks: 100,
        system_cpu_ticks: 1_000,
        disk_read_bytes: 1_000,
        disk_write_bytes: 2_000,
        ..StatusResourceCounters::default()
    };
    let current = StatusResourceCounters {
        available: true,
        process_cpu_ticks: 120,
        system_cpu_ticks: 1_200,
        disk_read_bytes: 3_000,
        disk_write_bytes: 5_000,
        ..StatusResourceCounters::default()
    };
    let snapshot = status_resource_snapshot(Some((previous, Duration::from_secs(2))), current);

    assert_eq!(snapshot.cpu_percent, Some(10.0));
    assert_eq!(snapshot.disk_read_bytes_per_second, 1_000);
    assert_eq!(snapshot.disk_write_bytes_per_second, 1_500);
}

#[test]
fn status_resource_history_aggregation_uses_mojo() {
    let mut history = StatusResourceHistory::default();
    let snapshot = StatusResourceSnapshot {
        available: true,
        process_count: 2,
        runtime_process_count: 1,
        cpu_percent: Some(12.5),
        resident_bytes: 512,
        disk_read_bytes_per_second: u64::MAX,
        disk_write_bytes_per_second: 1,
        network_rx_queue_bytes: 4,
        network_tx_queue_bytes: 6,
        ..StatusResourceSnapshot::default()
    };
    history.push(&snapshot);

    assert_eq!(history.cpu.iter().copied().collect::<Vec<_>>(), [13]);
    assert_eq!(history.memory.iter().copied().collect::<Vec<_>>(), [512]);
    assert_eq!(history.disk.iter().copied().collect::<Vec<_>>(), [u64::MAX]);
    assert_eq!(history.network.iter().copied().collect::<Vec<_>>(), [10]);
}

#[test]
fn status_resource_mojo_boundary_rejects_non_finite_elapsed_time() {
    let counters = prodex_mojo_core::info_render::InfoStatusResourceCounters {
        available: true,
        ..prodex_mojo_core::info_render::InfoStatusResourceCounters::default()
    };
    assert!(matches!(
        prodex_mojo_core::info_render::status_resource_metrics(
            Some((counters, f64::NAN)),
            counters,
        ),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    ));
}

#[test]
fn status_quota_gauge_uses_mojo_threshold_and_reset_label() {
    let gauge = prodex_mojo_core::info_render::format_status_quota_gauge(
        25,
        2,
        Some(1_060),
        1_000,
        Some("01-01 00:17"),
    )
    .unwrap();
    assert_eq!(gauge.band, 1);
    assert_eq!(gauge.ratio, 0.13);
    assert_eq!(
        gauge.label,
        "13% avg · pool 25% · reset in 1m (01-01 00:17)"
    );
}

#[test]
fn status_fields_mark_proc_resources_unavailable_instead_of_zero() {
    let overview = StatusOverview {
        updated_at: "now".to_string(),
        active_profile: "main".to_string(),
        runtime_profile: "main".to_string(),
        profile_count: 1,
        quota: StatusQuotaSummary::default(),
        five_hour_runway: None,
        weekly_runway: None,
        token_summary: InfoTokenUsageSummary::default(),
        token_history: Vec::new(),
        token_first_at: None,
        token_last_at: None,
        runtime_load: crate::InfoRuntimeLoadSummary::default(),
        runtime_process_count: 0,
    };
    let fields = status_fields(&overview, &StatusResourceSnapshot::default());

    for label in ["Processes", "Memory", "Network", "Disk I/O"] {
        let value = fields
            .iter()
            .find_map(|(field, value)| (field == label).then_some(value))
            .expect("status resource field should exist");
        assert_eq!(value, "unavailable");
    }
    assert_eq!(
        fields
            .iter()
            .find(|(label, _)| label == "Profile")
            .map(|(_, value)| value.as_str()),
        Some("runtime=main, configured=main, pool=1, quota-compatible=0, unavailable=0")
    );
}

#[test]
fn status_fields_keep_quota_token_and_resource_output_contract() {
    let mut overview = StatusOverview {
        updated_at: "now".to_string(),
        active_profile: "configured".to_string(),
        runtime_profile: "runtime".to_string(),
        profile_count: 3,
        quota: StatusQuotaSummary {
            compatible_profiles: 2,
            unavailable_profiles: 1,
            five_hour: StatusQuotaWindow {
                profiles: 2,
                total_remaining: 90,
                earliest_reset_at: None,
            },
            weekly: StatusQuotaWindow::default(),
        },
        five_hour_runway: None,
        weekly_runway: None,
        token_summary: InfoTokenUsageSummary {
            log_count: 1,
            event_count: 2,
            total: crate::reports::InfoTokenUsageCounts {
                input_tokens: 200,
                cached_input_tokens: 50,
                output_tokens: 100,
                reasoning_tokens: 20,
            },
            ..InfoTokenUsageSummary::default()
        },
        token_history: vec![1, 2, 3],
        token_first_at: Some("first".to_string()),
        token_last_at: Some("last".to_string()),
        runtime_load: crate::InfoRuntimeLoadSummary::default(),
        runtime_process_count: 1,
    };
    let resources = StatusResourceSnapshot {
        available: true,
        process_count: 3,
        runtime_process_count: 1,
        cpu_percent: Some(12.5),
        resident_bytes: 512,
        memory_total_bytes: 1_024,
        socket_count: 2,
        network_rx_queue_bytes: 1_024,
        network_tx_queue_bytes: 1_536,
        disk_read_bytes: 2_048,
        disk_write_bytes: 4_096,
        disk_read_bytes_per_second: 1_024,
        disk_write_bytes_per_second: 1_536,
    };
    let fields = status_fields(&overview, &resources);
    let value = |name: &str| {
        fields
            .iter()
            .find(|(label, _)| label == name)
            .map(|(_, value)| value.as_str())
            .unwrap_or_else(|| panic!("missing status field {name}"))
    };

    assert_eq!(
        value("Profile"),
        "runtime=runtime, configured=configured, pool=3, quota-compatible=2, unavailable=1"
    );
    assert_eq!(value("5h quota"), "90% across 2 profile(s)");
    assert_eq!(
        value("5h runway"),
        "Unavailable (no recent quota decay observed in active runtime logs)"
    );
    assert_eq!(value("Weekly quota"), "Unavailable");
    assert_eq!(
        value("Token efficiency"),
        "cache hit 25.0% · output share 33.3%"
    );
    assert_eq!(value("Token history"), "▃▅█ first → last");
    assert_eq!(value("Processes"), "3 total, 1 runtime; CPU 12.5%");
    assert_eq!(value("Memory"), "512 B (50.0% host)");
    assert_eq!(
        value("Network"),
        "2 sockets; RX queue 1.0 KiB, TX queue 1.5 KiB"
    );
    assert_eq!(
        value("Disk I/O"),
        "read 2.0 KiB total (1.0 KiB/s), write 4.0 KiB total (1.5 KiB/s)"
    );
    assert_eq!(value("Updated"), "now");

    overview.quota.five_hour.total_remaining = 0;
    let fields = status_fields(&overview, &resources);
    assert_eq!(
        fields
            .iter()
            .find(|(label, _)| label == "5h runway")
            .map(|(_, value)| value.as_str()),
        Some("Exhausted")
    );
}

#[test]
fn status_field_mojo_boundary_rejects_non_finite_cpu_input() {
    let result = prodex_mojo_core::info_render::format_status_fields(
        prodex_mojo_core::info_render::InfoStatusFields {
            runtime_profile: "runtime",
            active_profile: "active",
            profile_count: 1,
            quota_compatible_profiles: 1,
            unavailable_profiles: 0,
            five_hour_quota: "quota",
            five_hour_runway: "runway",
            weekly_quota: "quota",
            weekly_runway: "runway",
            token_usage_summary: "tokens",
            token_input: 0,
            token_cached_input: 0,
            token_output: 0,
            token_history_text: "-",
            token_first_at: None,
            token_last_at: None,
            resources: prodex_mojo_core::info_render::InfoStatusResources {
                available: true,
                process_count: 1,
                cpu_percent: Some(f64::NAN),
                ..prodex_mojo_core::info_render::InfoStatusResources::default()
            },
            recent_load: "load",
            updated_at: "now",
        },
    );
    assert!(matches!(
        result,
        Err(prodex_mojo_core::MojoError::InvalidInput)
    ));
}

#[test]
fn quota_summary_keeps_cache_precedence_and_window_semantics() {
    let now = 1_000;
    let (reports, snapshots) = status_quota_fixture(now);
    let expected = StatusQuotaSummary {
        compatible_profiles: 7,
        unavailable_profiles: 4,
        five_hour: StatusQuotaWindow {
            profiles: 2,
            total_remaining: 130,
            earliest_reset_at: Some(1_200),
        },
        weekly: StatusQuotaWindow {
            profiles: 2,
            total_remaining: 70,
            earliest_reset_at: Some(1_800),
        },
    };

    let summary = status_quota_from_reports(&reports, &snapshots, now).unwrap();
    assert_eq!(summary, expected);

    let mut reversed = reports;
    reversed.reverse();
    assert_eq!(
        status_quota_from_reports(&reversed, &snapshots, now).unwrap(),
        expected
    );
}

#[test]
fn network_queue_parser_filters_prodex_socket_inodes() {
    let table = concat!(
        "sl local_address rem_address st tx_queue:rx_queue tr tm->when retrnsmt uid timeout inode\n",
        "0: 0100007F:1F90 00000000:0000 0A 00000010:00000020 00:00000000 00000000 1000 0 42\n",
        "1: 0100007F:1F91 00000000:0000 0A 00000100:00000200 00:00000000 00000000 1000 0 99\n",
    );
    assert_eq!(
        parse_network_queues(table, &HashSet::from([42])),
        (0x20, 0x10)
    );
}

#[test]
fn token_history_is_chronological_and_bounded() {
    let event = |timestamp: &str, input_tokens| InfoTokenUsageEvent {
        timestamp: timestamp.to_string(),
        profile: "main".to_string(),
        input_tokens,
        output_tokens: 5,
        ..InfoTokenUsageEvent::default()
    };
    let events = vec![event("1", 10), event("2", 20), event("3", 30)];
    let history = status_token_history(&events).unwrap();
    assert_eq!(history.history, vec![15, 25, 35]);
    assert_eq!(history.first_at.as_deref(), Some("1"));
    assert_eq!(history.last_at.as_deref(), Some("3"));
    assert_eq!(history.latest_profile.as_deref(), Some("main"));
    assert_eq!(text_sparkline(&[1, 2, 3]).unwrap().chars().count(), 3);
}

#[test]
fn token_history_drops_oldest_event_at_mojo_bound() {
    let events = (0..65)
        .map(|index| InfoTokenUsageEvent {
            timestamp: format!("{index:02}"),
            profile: "main".to_string(),
            input_tokens: index,
            ..InfoTokenUsageEvent::default()
        })
        .collect::<Vec<_>>();
    let history = status_token_history(&events).unwrap();
    assert_eq!(history.history.len(), 64);
    assert_eq!(history.history.first(), Some(&1));
    assert_eq!(history.history.last(), Some(&64));
}

#[test]
fn status_profile_filter_preserves_input_order_through_mojo() {
    assert_eq!(
        status_profile_indices(&[false, true, true, false]).unwrap(),
        [1, 2]
    );
    assert!(status_profile_indices(&[]).unwrap().is_empty());
}

#[test]
fn status_runtime_profile_precedence_uses_mojo_callpath() {
    use prodex_mojo_core::info_render::InfoStatusRuntimeCandidate;

    let candidates = [
        InfoStatusRuntimeCandidate {
            timestamp: 10,
            profile: "old",
        },
        InfoStatusRuntimeCandidate {
            timestamp: 20,
            profile: "tie-first",
        },
        InfoStatusRuntimeCandidate {
            timestamp: 20,
            profile: "tie-last",
        },
    ];
    assert_eq!(
        status_runtime_profile(0, "active", Some("token"), &candidates).unwrap(),
        "active"
    );
    assert_eq!(
        status_runtime_profile(1, "active", Some("token"), &candidates).unwrap(),
        "tie-last"
    );
    assert_eq!(
        status_runtime_profile(1, "active", Some("token"), &[]).unwrap(),
        "token"
    );
    assert_eq!(
        status_runtime_profile(1, "active", None, &[]).unwrap(),
        "active"
    );
}

#[test]
fn dashboard_renders_at_wide_standard_and_compact_sizes() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let overview = StatusOverview {
        updated_at: "2026-07-13 20:00:00".to_string(),
        active_profile: "main".to_string(),
        runtime_profile: "main".to_string(),
        profile_count: 2,
        quota: StatusQuotaSummary {
            compatible_profiles: 2,
            five_hour: StatusQuotaWindow {
                profiles: 2,
                total_remaining: 140,
                earliest_reset_at: Some(2_000),
            },
            weekly: StatusQuotaWindow {
                profiles: 2,
                total_remaining: 120,
                earliest_reset_at: Some(10_000),
            },
            ..StatusQuotaSummary::default()
        },
        five_hour_runway: None,
        weekly_runway: None,
        token_summary: InfoTokenUsageSummary::default(),
        token_history: vec![10, 20, 30],
        token_first_at: Some("first".to_string()),
        token_last_at: Some("last".to_string()),
        runtime_load: crate::InfoRuntimeLoadSummary::default(),
        runtime_process_count: 1,
    };
    let resources = StatusResourceSnapshot {
        available: true,
        process_count: 2,
        runtime_process_count: 1,
        cpu_percent: Some(12.5),
        resident_bytes: 64 * 1024 * 1024,
        memory_total_bytes: 1024 * 1024 * 1024,
        ..StatusResourceSnapshot::default()
    };

    for (width, height) in [(120, 40), (80, 24), (60, 12)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
        terminal
            .draw(|frame| {
                render_status_dashboard(
                    frame,
                    Some(&overview),
                    &resources,
                    &StatusResourceHistory::default(),
                    None,
                    false,
                )
            })
            .expect("status dashboard should render");
    }
}

#[test]
fn status_keyboard_contract_recognizes_quit_keys() {
    for (code, modifiers) in [
        (KeyCode::Char('q'), KeyModifiers::NONE),
        (KeyCode::Esc, KeyModifiers::NONE),
        (KeyCode::Char('c'), KeyModifiers::CONTROL),
        (KeyCode::Char('z'), KeyModifiers::CONTROL),
    ] {
        let key = crossterm::event::KeyEvent::new(code, modifiers);
        assert!(status_quit_key(&key), "{key:?} should quit");
    }
}
