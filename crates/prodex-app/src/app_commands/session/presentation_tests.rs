use super::{SessionReport, session_report_tui_item, session_scroll_lines};
use std::path::Path;

#[test]
fn session_tui_rows_preserve_original_complete_spans() {
    let mut missing = SessionReport::from_path(Path::new("/home/test-user/synthetic.jsonl"), 0);
    missing.id = "session-missing".to_string();
    missing.updated_at = None;
    let mut empty = missing.clone();
    empty.id = String::new();
    empty.thread_name = Some(String::new());
    empty.updated_at = Some(String::new());
    empty.cwd = Some(String::new());
    empty.profile = Some(String::new());
    empty.model_provider = Some(String::new());
    let mut unicode = empty.clone();
    unicode.id = "session-雪".to_string();
    unicode.thread_name = Some(" \t雪\n保持  ".to_string());
    unicode.updated_at = Some("2026-10-23T00:00:00Z".to_string());
    unicode.cwd = Some("/home/test-user/雪\nproject".to_string());
    unicode.profile = Some("profile-β".to_string());
    unicode.model_provider = Some("provider-\"test".to_string());
    let mut blank = missing.clone();
    blank.id = "session-blank".to_string();
    blank.thread_name = Some("\u{0085}\u{00a0}\u{3000} ".to_string());
    let mut zero_width = missing.clone();
    zero_width.id = "session-zero-width".to_string();
    zero_width.thread_name = Some("\u{200b}\u{feff}".to_string());
    zero_width.cwd = Some("".to_string());
    let mut plain = missing.clone();
    plain.id = "session-plain".to_string();
    plain.thread_name = Some("Build UI".to_string());
    plain.updated_at = Some("2026-10-23T00:00:00Z".to_string());
    plain.cwd = Some("/home/test-user/project".to_string());
    plain.profile = Some("synthetic".to_string());
    plain.model_provider = Some("openai".to_string());
    let reports = [missing, empty, unicode, blank, zero_width, plain];
    let snapshot = serde_json::json!({
        "inline": reports.iter().map(|report| format!("{:?}", session_report_tui_item(report).unwrap())).collect::<Vec<_>>(),
        "scroll": format!("{:?}", session_scroll_lines(&reports).unwrap()),
        "empty_scroll": format!("{:?}", session_scroll_lines(&[]).unwrap()),
    });
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("presentation_baseline.json")).unwrap();
    assert_eq!(snapshot, expected);
}
