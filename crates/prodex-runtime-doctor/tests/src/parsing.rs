use super::*;

#[test]
fn runtime_doctor_parse_message_fields_match_fixed_values() {
    let fields = runtime_doctor_parse_message_fields(
        r#"selection_pick request=7 transport=http profile="alpha beta" note="say \"yes\"" city=東京 empty="" malformed="unterminated"#,
    );

    let expected = [
        ("city", "東京"),
        ("empty", ""),
        ("malformed", "unterminated"),
        ("note", "say \"yes\""),
        ("profile", "alpha beta"),
        ("request", "7"),
        ("transport", "http"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value.to_string()))
    .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(fields, expected);
}

#[test]
fn runtime_doctor_prefers_json_event_and_fields() {
    let log = br#"{"timestamp":"2026-05-12T00:00:00Z","message":"runtime_proxy_queue_overloaded lane=responses active=1","event":"runtime_proxy_lane_limit_reached","fields":{"lane":"compact","active":6,"overflow":false}}"#;

    let summary = summarize_runtime_log_tail(log);

    assert_eq!(
        summary
            .marker_counts
            .get("runtime_proxy_lane_limit_reached")
            .copied(),
        Some(1)
    );
    assert_eq!(
        summary
            .marker_counts
            .get("runtime_proxy_queue_overloaded")
            .copied(),
        None
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("runtime_proxy_lane_limit_reached")
            .and_then(|fields| fields.get("lane"))
            .map(String::as_str),
        Some("compact")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("runtime_proxy_lane_limit_reached")
            .and_then(|fields| fields.get("active"))
            .map(String::as_str),
        Some("6")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("runtime_proxy_lane_limit_reached")
            .and_then(|fields| fields.get("overflow"))
            .map(String::as_str),
        Some("false")
    );
}

#[test]
fn runtime_doctor_finds_known_marker_after_unclassified_message_prefix() {
    let line = RuntimeDoctorParsedLogLine::new(
        "[2026-05-12 00:00:00Z] notice request_id=req-1 selection_pick profile=alpha",
    );

    assert_eq!(line.marker_name().as_deref(), Some("selection_pick"));
    assert_eq!(
        line.fields(),
        std::collections::BTreeMap::from([
            ("profile".to_string(), "alpha".to_string()),
            ("request_id".to_string(), "req-1".to_string()),
        ])
    );
}

#[test]
fn runtime_doctor_uses_message_event_when_json_event_is_unknown() {
    let log = br#"{"event":"unknown_marker","message":"selection_pick profile=alpha"}"#;

    let summary = summarize_runtime_log_tail(log);

    assert_eq!(
        summary.marker_counts.get("selection_pick").copied(),
        Some(1)
    );
    assert_eq!(summary.marker_counts.get("unknown_marker").copied(), None);
}

#[test]
fn runtime_doctor_redacts_secret_fields_and_terminal_controls() {
    let json_line = r#"{"timestamp":"2026-05-12T00:00:00Z","event":"runtime_proxy_lane_limit_reached","fields":{"route":"/v1/responses\u001b[31m","authorization":"Bearer fixture-secret-sentinel"}}"#;
    let text_line = format!(
        "[2026-05-12 00:00:01.000 +00:00] stream_read_error route=\"/v1/responses{}[31m\" authorization=\"Bearer fixture-secret-sentinel\"",
        '\u{1b}'
    );
    let log = format!("{json_line}\n{text_line}");

    let summary = summarize_runtime_log_tail(log.as_bytes());

    let json_fields = summary
        .marker_last_fields
        .get("runtime_proxy_lane_limit_reached")
        .unwrap();
    assert_eq!(json_fields["authorization"], "<redacted>");
    assert!(
        !json_fields["route"]
            .chars()
            .any(|character| character.is_control() || character == '\u{7f}')
    );
    let text_fields = summary.marker_last_fields.get("stream_read_error").unwrap();
    assert_eq!(text_fields["authorization"], "<redacted>");
    assert!(
        !text_fields["route"]
            .chars()
            .any(|character| character.is_control() || character == '\u{7f}')
    );
    let rendered = runtime_doctor_json_value(&summary).to_string();
    assert!(!rendered.contains("fixture-secret-sentinel"), "{rendered}");
    assert!(
        !rendered
            .chars()
            .any(|character| character.is_control() || character == '\u{7f}')
    );
}

#[test]
fn runtime_doctor_falls_back_to_typed_text_parser() {
    let log = br#"[2026-05-12 00:00:00.000 +00:00] stream_read_error request=7 transport=http error="failed with spaces""#;

    let summary = summarize_runtime_log_tail(log);

    assert_eq!(
        summary.marker_counts.get("stream_read_error").copied(),
        Some(1)
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("stream_read_error")
            .and_then(|fields| fields.get("error"))
            .map(String::as_str),
        Some("failed with spaces")
    );
}

#[test]
fn runtime_doctor_summarizes_marker_context_by_route_lane_and_profile() {
    let log = br#"[2026-05-12 00:00:00.000 +00:00] runtime_proxy_lane_limit_reached lane=compact route=/responses/compact profile=alpha active=4
[2026-05-12 00:00:01.000 +00:00] runtime_proxy_lane_limit_reached lane=compact route=/responses/compact profile=beta active=5
[2026-05-12 00:00:02.000 +00:00] profile_inflight_saturated route=responses profile=alpha active=8
"#;

    let summary = summarize_runtime_log_tail(log);

    let lane_limit = summary
        .marker_context_summary
        .iter()
        .find(|entry| entry.marker == "runtime_proxy_lane_limit_reached")
        .expect("lane limit marker context should be summarized");
    assert_eq!(lane_limit.total, 2);
    assert_eq!(lane_limit.lanes.get("compact").copied(), Some(2));
    assert_eq!(
        lane_limit.routes.get("/responses/compact").copied(),
        Some(2)
    );
    assert_eq!(lane_limit.profiles.get("alpha").copied(), Some(1));
    assert_eq!(lane_limit.profiles.get("beta").copied(), Some(1));

    let inflight = summary
        .marker_context_summary
        .iter()
        .find(|entry| entry.marker == "profile_inflight_saturated")
        .expect("profile inflight marker context should be summarized");
    assert_eq!(inflight.total, 1);
    assert_eq!(inflight.routes.get("responses").copied(), Some(1));
    assert_eq!(inflight.profiles.get("alpha").copied(), Some(1));
}
