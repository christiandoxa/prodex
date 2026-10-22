use super::{ParsedOperationalEvent, operational_event_from_runtime_line};
use std::collections::BTreeMap;

fn parsed(event: &str, fields: &str) -> Option<ParsedOperationalEvent> {
    operational_event_from_runtime_line(&format!(
        "[2026-01-10 11:00:00.000 +00:00] {event} {fields}"
    ))
    .unwrap()
}

#[test]
fn compiled_operational_selection_suppresses_payloads_and_unswitched_profile_commits() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }
    for event in [
        "stream_payload",
        "upstream_payload",
        "token_usage",
        "token_usage_progress",
    ] {
        assert!(parsed(event, "request=17").is_none());
    }
    for fields in ["", "switched=false", "switched=TRUE", "switched=1"] {
        assert!(parsed("profile_commit", fields).is_none());
    }
    let selected = parsed("profile_commit", "switched=true request=17").unwrap();
    assert_eq!(selected.transcript.source, "route");
    assert!(selected.load.is_none());
}

#[test]
fn compiled_operational_selection_preserves_routine_fields_and_sanitizes_only_endpoint_values() {
    let selected = parsed("profile_inflight_saturated",
        "request=17 profile=fixture route=responses lane=responses transport=http context=fresh provider=openai model=fixture limit=8 hard_limit=16 reason=busy path=https://example.com/responses?fake=1#fragment ignored=excluded").unwrap();
    assert_eq!(selected.transcript.source, "load");
    let load = selected.load.unwrap();
    assert_eq!(
        load.fields,
        BTreeMap::from([
            ("profile".into(), "fixture".into()),
            ("route".into(), "responses".into()),
            ("lane".into(), "responses".into()),
            ("transport".into(), "http".into()),
            ("context".into(), "fresh".into()),
            ("provider".into(), "openai".into()),
            ("model".into(), "fixture".into()),
            ("limit".into(), "8".into()),
            ("hard_limit".into(), "16".into()),
            ("reason".into(), "busy".into()),
            ("path".into(), "/responses".into()),
        ])
    );
    assert!(load.run_id.is_some());
    assert!(!load.event.text.contains("fake=1"));
    for fields in ["path=/responses?fake=1#fragment", "path=invalid"] {
        let selected = parsed("runtime_proxy_lane_limit_reached", fields).unwrap();
        let expected = if fields.starts_with("path=/") {
            "/responses"
        } else {
            "upstream"
        };
        assert_eq!(
            selected
                .load
                .unwrap()
                .fields
                .get("path")
                .map(String::as_str),
            Some(expected)
        );
    }
}

#[test]
fn compiled_operational_selection_escalates_nonroutine_load_and_keeps_dynamic_interest() {
    for event in [
        "runtime_proxy_queue_overloaded",
        "websocket_connect_overflow_rejected",
    ] {
        let selected = parsed(event, "profile=fixture reason=busy").unwrap();
        assert_eq!(selected.transcript.source, "error");
        assert!(selected.load.is_none());
    }
    assert!(parsed("smart_context_prepare_fallback", "decision=pass_through").is_none());
    assert!(
        parsed(
            "compat_request_surface",
            "tool_surface=none family=codex continuation=none"
        )
        .is_none()
    );
    let selected = parsed("compat_request_surface", "tool_surface=mcp,functions").unwrap();
    assert_eq!(selected.transcript.source, "mcp");
    let selected = parsed("new_fixture_event", "ignored=excluded").unwrap();
    assert_eq!(selected.transcript.source, "event");
    assert!(!selected.transcript.text.contains("excluded"));
}
