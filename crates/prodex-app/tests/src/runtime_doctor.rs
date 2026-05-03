use super::*;

#[test]
fn summarize_runtime_log_tail_understands_json_lines() {
    let tail = br#"{"timestamp":"2026-04-08 10:00:00.000 +00:00","message":"request=7 profile_health profile=main route=responses score=4","fields":{"request":"7","profile":"main","route":"responses","score":"4"}}"#;
    let summary = summarize_runtime_log_tail(tail);

    assert_eq!(summary.line_count, 1);
    assert_eq!(
        summary.marker_counts.get("profile_health").copied(),
        Some(1)
    );
    assert_eq!(
        summary.first_timestamp.as_deref(),
        Some("2026-04-08 10:00:00.000 +00:00")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("profile_health")
            .and_then(|fields| fields.get("profile"))
            .map(String::as_str),
        Some("main")
    );
}
