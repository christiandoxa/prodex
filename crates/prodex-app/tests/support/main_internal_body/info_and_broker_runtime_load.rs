use super::*;

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
