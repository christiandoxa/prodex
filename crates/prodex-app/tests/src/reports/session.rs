//! App-owned session report tests.

use super::*;

#[test]
fn renders_session_reports_from_store_model() {
    let report = SessionReport::from_path(std::path::Path::new("/tmp/session-a.jsonl"), 0);

    let rendered = render_session_reports_text(&[report]);

    assert!(rendered.contains("session-a"));
}

#[test]
fn json_output_preserves_mojo_store_order_at_cli_report_boundary() {
    let mut reports = [
        SessionReport::from_path(std::path::Path::new("/home/test-user/session-z.jsonl"), 7),
        SessionReport::from_path(std::path::Path::new("/home/test-user/session-a.jsonl"), 7),
    ];
    prodex_session_store::sort_session_reports(&mut reports);

    let output = render_session_reports_output(&reports, true, "No sessions found").unwrap();
    let json: Vec<serde_json::Value> = serde_json::from_str(&output).unwrap();

    assert_eq!(
        json.iter()
            .map(|report| report["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["session-a", "session-z"]
    );
}
