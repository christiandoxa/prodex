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
fn remembers_the_latest_turn_model_and_reasoning_effort() {
    let mut report = SessionReport::from_path(Path::new("/tmp/session-settings.jsonl"), 0);
    apply_session_json_lines(
        &mut report,
        [
            r#"{"timestamp":"2026-04-29T11:59:00Z","type":"session_meta","payload":{"id":"session-settings"}}"#,
            r#"{"timestamp":"2026-04-29T12:00:00Z","type":"turn_context","payload":{"model":"gpt-5.2-codex","effort":"medium"}}"#,
            r#"{"timestamp":"2026-04-29T12:02:00Z","type":"turn_context","payload":{"model":"gpt-5.6-luna","effort":"max"}}"#,
            r#"{"timestamp":"2026-04-29T12:03:00Z","type":"response_item","payload":{"id":"response-item","model":"should-not-win"}}"#,
        ],
    );

    assert_eq!(report.id, "session-settings");
    assert_eq!(report.last_model(), Some("gpt-5.6-luna"));
    assert_eq!(report.last_reasoning_effort(), Some("max"));
}

#[test]
fn sorting_is_stable_for_equal_timestamps() {
    let mut path_b = SessionReport::from_path(Path::new("/tmp/b.jsonl"), 10);
    path_b.thread_name = Some("path-b".to_string());
    let mut id_a_path_b = SessionReport::from_path(Path::new("/tmp/other.jsonl"), 10);
    id_a_path_b.id = "a".to_string();
    id_a_path_b.path = "/tmp/b".to_string();
    id_a_path_b.thread_name = Some("id-a-path-b".to_string());
    let mut id_a_path_a_first = SessionReport::from_path(Path::new("/tmp/first.jsonl"), 10);
    id_a_path_a_first.id = "a".to_string();
    id_a_path_a_first.path = "/tmp/a".to_string();
    id_a_path_a_first.thread_name = Some("first-equal-key".to_string());
    let mut id_a_path_a_second = id_a_path_a_first.clone();
    id_a_path_a_second.thread_name = Some("second-equal-key".to_string());
    let mut newest = SessionReport::from_path(Path::new("/tmp/new.jsonl"), 20);
    newest.thread_name = Some("newest".to_string());
    let mut reports = [
        path_b,
        id_a_path_b,
        id_a_path_a_first,
        id_a_path_a_second,
        newest,
    ];

    sort_session_reports(&mut reports);

    assert_eq!(
        reports
            .iter()
            .map(|report| report.id.as_str())
            .collect::<Vec<_>>(),
        ["new", "a", "a", "a", "b"]
    );
    assert_eq!(
        reports
            .iter()
            .map(|report| report.thread_name.as_deref().unwrap())
            .collect::<Vec<_>>(),
        [
            "newest",
            "first-equal-key",
            "second-equal-key",
            "id-a-path-b",
            "path-b",
        ]
    );
}

#[test]
fn metadata_updates_and_numeric_timestamp_fallback_follow_mojo_plan() {
    let mut report = SessionReport::from_path(Path::new("/tmp/original.jsonl"), 7);
    apply_session_json_line(
        &mut report,
        r#"{"type":"response_item","payload":{"id":"ignored"}}"#,
    );
    assert_eq!(report.id, "original");

    for (line, expected_epoch) in [
        (
            r#"{"updated_at":101,"ts":102,"timestamp":103,"payload":{"updated_at":104,"ts":105,"timestamp":106}}"#,
            101,
        ),
        (
            r#"{"updated_at":1.5,"ts":102,"timestamp":103,"payload":{"updated_at":104,"ts":105,"timestamp":106}}"#,
            102,
        ),
        (
            r#"{"updated_at":1.5,"ts":"ignored","timestamp":103,"payload":{"updated_at":104,"ts":105,"timestamp":106}}"#,
            103,
        ),
        (
            r#"{"updated_at":null,"ts":null,"timestamp":null,"payload":{"updated_at":104,"ts":105,"timestamp":106}}"#,
            104,
        ),
        (
            r#"{"updated_at":null,"ts":null,"timestamp":null,"payload":{"updated_at":1.5,"ts":105,"timestamp":106}}"#,
            105,
        ),
        (
            r#"{"updated_at":null,"ts":null,"timestamp":null,"payload":{"updated_at":1.5,"ts":null,"timestamp":106}}"#,
            106,
        ),
    ] {
        apply_session_json_line(&mut report, line);
        assert_eq!(report.updated_sort_key, expected_epoch, "{line}");
        assert_eq!(
            report.updated_at.as_deref(),
            Some(format_epoch(expected_epoch).as_str()),
            "{line}"
        );
    }

    apply_session_json_line(&mut report, r#"{"payload":{"id":"allowed-without-type"}}"#);
    assert_eq!(report.id, "allowed-without-type");
}

#[test]
fn invalid_string_timestamp_keeps_sort_key_and_blocks_numeric_fallback() {
    let mut report = SessionReport::from_path(Path::new("/tmp/timestamp.jsonl"), 7);
    apply_session_json_line(
        &mut report,
        r#"{"updated_at":"1970-01-01T01:00:00+01:00","ts":88}"#,
    );
    assert_eq!(report.updated_sort_key, 0);

    apply_session_json_line(
        &mut report,
        r#"{"updated_at":" 1970-01-01T00:00:01Z ","ts":88}"#,
    );
    assert_eq!(report.updated_sort_key, 1);
    assert_eq!(report.updated_at.as_deref(), Some("1970-01-01T00:00:01Z"));

    apply_session_json_line(&mut report, r#"{"updated_at":"not-a-timestamp","ts":99}"#);
    assert_eq!(report.updated_sort_key, 1);
    assert_eq!(report.updated_at.as_deref(), Some("not-a-timestamp"));
}

#[test]
fn session_metadata_mojo_preserves_blank_type_and_decoded_whitespace_precedence() {
    let mut report = SessionReport::from_path(Path::new("/tmp/original.jsonl"), 0);
    apply_session_json_line(
        &mut report,
        r#"{"type":"   ","payload":{"id":"must-not-replace","thread_name":" ","title":"Fallback title","cwd":"　","workdir":"/tmp/fallback"}}"#,
    );

    assert_eq!(report.id, "original");
    assert_eq!(report.thread_name.as_deref(), Some("Fallback title"));
    assert_eq!(report.cwd.as_deref(), Some("/tmp/fallback"));
}

#[test]
fn session_metadata_mojo_keeps_nested_precedence_and_model_provider_fallbacks() {
    let mut report = SessionReport::from_path(Path::new("/tmp/nested.jsonl"), 0);
    apply_session_json_line(
        &mut report,
        r#"{"type":"session_meta","payload":{"session_id":"nested-id","metadata":{"thread_name":"Nested title","cwd":"/tmp/nested","model_provider":"nested-provider"},"source":{"subagent":{"thread_spawn":{"parent_thread_id":"parent-nested"}}}}}"#,
    );

    assert_eq!(report.id, "nested-id");
    assert_eq!(report.thread_name.as_deref(), Some("Nested title"));
    assert_eq!(report.cwd.as_deref(), Some("/tmp/nested"));
    assert_eq!(report.model_provider.as_deref(), Some("nested-provider"));
    assert_eq!(report.parent_thread_id.as_deref(), Some("parent-nested"));
}
