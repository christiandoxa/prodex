use super::*;

#[test]
fn cleanup_summary_total_and_merge_count_all_fields() {
    let left = ProdexCleanupSummary {
        duplicate_profiles_removed: 1,
        runtime_logs_removed: 2,
        chat_history_entries_removed: 3,
        ..ProdexCleanupSummary::default()
    };
    let right = ProdexCleanupSummary {
        stale_login_dirs_removed: 4,
        dead_runtime_broker_registries_removed: 5,
        ..ProdexCleanupSummary::default()
    };

    let merged = left.merge(right);

    assert_eq!(merged.total_removed(), 15);
    assert_eq!(merged.runtime_logs_removed, 2);
    assert_eq!(merged.stale_login_dirs_removed, 4);
}

#[test]
fn history_epoch_accepts_seconds_millis_and_rfc3339() {
    assert_eq!(
        prodex_history_line_epoch(r#"{"timestamp":1700000000}"#),
        Some(1_700_000_000)
    );
    assert_eq!(
        prodex_history_line_epoch(r#"{"timestamp":1700000000000}"#),
        Some(1_700_000_000)
    );
    assert_eq!(
        prodex_history_line_epoch(r#"{"created_at":"2024-01-01T00:00:00Z"}"#),
        Some(1_704_067_200)
    );
}
