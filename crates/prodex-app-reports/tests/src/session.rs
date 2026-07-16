use super::*;

#[test]
fn renders_session_reports_from_store_model() {
    let report = SessionReport::from_path(std::path::Path::new("/tmp/session-a.jsonl"), 0);

    let rendered = render_session_reports_text(&[report]);

    assert!(rendered.contains("session-a"));
}
