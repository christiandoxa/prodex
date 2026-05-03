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
