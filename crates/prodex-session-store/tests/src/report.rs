use super::*;

#[test]
fn parses_session_metadata_from_jsonl_values() {
    let mut report = SessionReport::from_path(Path::new("/tmp/session-a.jsonl"), 0);
    apply_session_json_line(
        &mut report,
        r#"{"timestamp":"2026-04-29T12:00:00Z","type":"session_meta","payload":{"id":"sess-a","thread_name":"Issue triage","cwd":"/tmp/workspace"}}"#,
    );

    assert_eq!(report.id, "sess-a");
    assert_eq!(report.thread_name.as_deref(), Some("Issue triage"));
    assert_eq!(report.cwd.as_deref(), Some("/tmp/workspace"));
    assert_eq!(report.updated_at.as_deref(), Some("2026-04-29T12:00:00Z"));
}

#[test]
fn parses_subagent_parent_thread_id() {
    let mut report = SessionReport::from_path(Path::new("/tmp/child.jsonl"), 0);
    apply_session_json_line(
        &mut report,
        r#"{"timestamp":"2026-04-29T12:00:00Z","type":"session_meta","payload":{"id":"child","source":{"subagent":{"thread_spawn":{"parent_thread_id":"parent"}}}}}"#,
    );

    assert!(report.is_subagent());
    assert_eq!(report.parent_thread_id.as_deref(), Some("parent"));
}

#[test]
fn sorting_is_stable_for_equal_timestamps() {
    let mut reports = [
        SessionReport::from_path(Path::new("/tmp/b.jsonl"), 10),
        SessionReport::from_path(Path::new("/tmp/a.jsonl"), 10),
        SessionReport::from_path(Path::new("/tmp/new.jsonl"), 20),
    ];

    sort_session_reports(&mut reports);

    assert_eq!(
        reports
            .iter()
            .map(|report| report.id.as_str())
            .collect::<Vec<_>>(),
        ["new", "a", "b"]
    );
}
