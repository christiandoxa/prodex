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

#[test]
fn session_staleness_uses_latest_jsonl_timestamp_not_path_date() {
    let root = housekeeping_test_temp_dir("session-latest");
    let session = root.join("sessions/2026/04/10/live-parent.jsonl");
    fs::create_dir_all(session.parent().unwrap()).expect("session dir should exist");
    fs::write(
        &session,
        "{\"timestamp\":\"2026-04-10T12:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"live-parent\"}}\n\
         {\"timestamp\":\"2026-05-08T12:00:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\"}}\n",
    )
    .expect("session should write");

    assert!(!prodex_session_file_is_stale(
        &session,
        housekeeping_epoch("2026-04-15T00:00:00Z")
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_staleness_prunes_when_latest_jsonl_timestamp_is_old() {
    let root = housekeeping_test_temp_dir("session-old");
    let session = root.join("sessions/2026/04/10/old-parent.jsonl");
    fs::create_dir_all(session.parent().unwrap()).expect("session dir should exist");
    fs::write(
        &session,
        "{\"timestamp\":\"2026-04-10T12:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"old-parent\"}}\n\
         {\"timestamp\":\"2026-04-12T12:00:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\"}}\n",
    )
    .expect("session should write");

    assert!(prodex_session_file_is_stale(
        &session,
        housekeeping_epoch("2026-04-15T00:00:00Z")
    ));

    let _ = fs::remove_dir_all(root);
}

fn housekeeping_epoch(value: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(value)
        .expect("timestamp should parse")
        .timestamp()
}

fn housekeeping_test_temp_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "prodex-housekeeping-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old temp dir should remove");
    }
    fs::create_dir_all(&root).expect("temp dir should create");
    root
}
