use super::*;

#[test]
fn runtime_log_envelopes_preserve_caller_header_and_unicode_whitespace_rules() {
    for (line, timestamp, message) in [
        (
            "[β] \u{2003}event profile=alpha] later\u{2003}",
            Some("β"),
            "event profile=alpha] later",
        ),
        ("prefix] event profile=alpha", None, "event profile=alpha"),
        ("[] ", None, ""),
        ("[] event", Some(""), "event"),
        ("[unterminated", None, "[unterminated"),
        ("[x]event", None, "[x]event"),
        ("\u{2003}event\u{2003}", None, "event"),
    ] {
        assert_eq!(
            info_runtime_line_timestamp(line).as_deref(),
            timestamp,
            "{line:?}"
        );
        assert_eq!(info_runtime_log_message(line), message, "{line:?}");
    }
    assert_eq!(
        info_runtime_line_timestamp(r#"{"timestamp":"json] time","message":"[raw] event"}"#)
            .as_deref(),
        Some("json] time")
    );
    assert_eq!(
        info_runtime_line_timestamp(r#"{"timestamp":42,"message":"[raw] event"}"#),
        None
    );
}

#[test]
fn runtime_load_preserves_per_log_counts_and_stable_combined_order() {
    let now = runtime_log_timestamp_epoch("[2026-06-01 00:10:00 +00:00] fixture").unwrap();
    let text = concat!(
        "profile_inflight profile=alpha count=5\n",
        "profile_inflight profile=βeta count=1\n",
        "profile_inflight profile=alpha count=2\n",
        "[2026-06-01 00:09:00 +00:00] selection_pick profile=βeta five_hour_remaining=5 weekly_remaining=50\n",
        "[2026-06-01 00:08:00 +00:00] selection_pick profile=alpha five_hour_remaining=2 weekly_remaining=20\n",
        "[2026-06-01 00:09:00 +00:00] selection_keep_current profile=alpha five_hour_remaining=3 weekly_remaining=30\n",
        "[2026-06-01 00:09:00 +00:00] selection_pick profile=alpha five_hour_remaining=4 weekly_remaining=40\n",
        "[2026-06-01 00:00:00 +00:00] selection_pick profile=old five_hour_remaining=0 weekly_remaining=0\n",
    );
    let single = collect_info_runtime_load_summary_from_texts(1, [text], now, 60, 120);
    assert_eq!(
        (
            single.log_count,
            single.active_inflight_units,
            single.recent_selection_events,
            single.recent_first_timestamp,
            single.recent_last_timestamp
        ),
        (1, 3, 3, Some(now - 60), Some(now - 60))
    );
    assert_eq!(
        single
            .observations
            .iter()
            .map(|row| row.five_hour_remaining)
            .collect::<Vec<_>>(),
        [2, 3, 4, 5]
    );
    let combined = collect_info_runtime_load_summary_from_texts(
        99,
        [text, "profile_inflight profile=alpha count=7"],
        now,
        60,
        120,
    );
    assert_eq!(
        (
            combined.log_count,
            combined.active_inflight_units,
            combined.recent_selection_events,
            combined.recent_first_timestamp,
            combined.recent_last_timestamp
        ),
        (99, 10, 3, Some(now - 60), Some(now - 60))
    );
    assert_eq!(
        combined
            .observations
            .iter()
            .map(|row| (
                row.timestamp,
                row.profile.as_str(),
                row.five_hour_remaining,
                row.weekly_remaining
            ))
            .collect::<Vec<_>>(),
        [
            (now - 120, "alpha", 2, 20),
            (now - 60, "alpha", 3, 30),
            (now - 60, "alpha", 4, 40),
            (now - 60, "βeta", 5, 50),
        ]
    );
}

#[test]
fn runtime_load_recent_statistics_remain_independent_of_lookback_selection() {
    let now = runtime_log_timestamp_epoch("[2026-06-01 00:10:00 +00:00] fixture").unwrap();
    let text = concat!(
        "[2026-06-01 00:08:00 +00:00] selection_pick profile=alpha five_hour_remaining=2 weekly_remaining=20\n",
        "[2026-06-01 00:09:00 +00:00] selection_pick profile=alpha five_hour_remaining=3 weekly_remaining=30\n",
    );
    let summary = collect_info_runtime_load_summary_from_texts(1, [text], now, 120, 60);
    assert_eq!(summary.observations.len(), 1);
    assert_eq!(
        (
            summary.recent_selection_events,
            summary.recent_first_timestamp,
            summary.recent_last_timestamp
        ),
        (2, Some(now - 120), Some(now - 60))
    );
}

#[test]
fn runtime_load_empty_logs_keep_absent_recent_timestamps() {
    let summary = collect_info_runtime_load_summary_from_texts(3, ["", "invalid record"], 0, 0, 0);
    assert_eq!(
        (
            summary.log_count,
            summary.active_inflight_units,
            summary.recent_selection_events,
            summary.recent_first_timestamp,
            summary.recent_last_timestamp
        ),
        (3, 0, 0, None, None)
    );
    assert!(summary.observations.is_empty());
}
