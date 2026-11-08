use super::*;

#[test]
fn newly_discovered_session_replays_complete_reasoning_tail_once() {
    let root = std::env::temp_dir().join(format!(
        "prodex-log-new-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("session.jsonl");
    let line = r#"{"timestamp":"2026-01-01T00:00:00Z","type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"REASONING_COMPLETE"}]}}"#;
    fs::write(&path, format!("{}\n{line}\n", "x".repeat(8192))).unwrap();
    let mut follower = FollowedLog::at_recent_tail(&path, 1024);
    assert!(follower.offset >= 8192);
    let lines = collect_new_followed_lines(&path, &mut follower).unwrap();
    assert_eq!(lines, vec![line]);
    assert!(
        collect_new_followed_lines(&path, &mut follower)
            .unwrap()
            .is_empty()
    );
    drop(follower);
    fs::remove_dir_all(root).unwrap();
}
