use super::*;

#[test]
fn runtime_doctor_parse_message_fields_handles_quoted_structured_values() {
    let fields = runtime_doctor_parse_message_fields(
        "stream_read_error request=7 transport=http error=\"failed with spaces\" empty=\"\"",
    );

    assert_eq!(fields.get("request").map(String::as_str), Some("7"));
    assert_eq!(
        fields.get("error").map(String::as_str),
        Some("failed with spaces")
    );
    assert_eq!(fields.get("empty").map(String::as_str), Some(""));
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
