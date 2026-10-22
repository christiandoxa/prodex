use super::*;
use std::collections::BTreeMap;

#[test]
fn structured_log_render_preserves_duplicates_and_separate_raw_field_map() {
    let event = RuntimeProxyLogEvent::new(
        "",
        [
            runtime_proxy_log_field("", "absent"),
            runtime_proxy_log_field("bad key", "absent"),
            runtime_proxy_log_field("a=b", "absent"),
            runtime_proxy_log_field("a\u{85}b", "absent"),
            runtime_proxy_log_field("clé", "one"),
            runtime_proxy_log_field("clé", "two"),
            runtime_proxy_log_field("empty", ""),
        ],
    );
    assert_eq!(event.render_message(), "clé=one clé=two empty=\"\"");
    assert_eq!(
        event.fields_map(),
        BTreeMap::from([
            ("".to_string(), "absent".to_string()),
            ("bad key".to_string(), "absent".to_string()),
            ("a=b".to_string(), "absent".to_string()),
            ("a\u{85}b".to_string(), "absent".to_string()),
            ("clé".to_string(), "two".to_string()),
            ("empty".to_string(), "".to_string()),
        ])
    );
}

#[test]
fn structured_log_render_preserves_empty_event_and_exact_separator_spacing() {
    for (event, fields, expected) in [
        ("", vec![], ""),
        ("", vec![runtime_proxy_log_field("bad\nkey", "hidden")], ""),
        ("\n", vec![], " "),
        (
            " ",
            vec![runtime_proxy_log_field("key", "value")],
            "  key=value",
        ),
        (
            "évent ",
            vec![runtime_proxy_log_field("key", "value")],
            "évent  key=value",
        ),
    ] {
        assert_eq!(
            runtime_proxy_structured_log_message(event, fields),
            expected
        );
    }
}

#[test]
fn structured_log_render_keeps_host_redaction_and_quoting_before_composition() {
    assert_eq!(
        runtime_proxy_structured_log_message(
            "event",
            [
                runtime_proxy_log_field("detail", "upstream_timeout"),
                runtime_proxy_log_field("detail", "arbitrary prose"),
                runtime_proxy_log_field("access_token", "fixture-secret-sentinel"),
                runtime_proxy_log_field("note", "line\nbreak"),
                runtime_proxy_log_field("note", "a\u{a0}b"),
                runtime_proxy_log_field("note", "\"quoted\"\\"),
            ],
        ),
        "event detail=upstream_timeout detail=<redacted> access_token=<redacted> note=\"line break\" note=a\u{a0}b note=\"\\\"quoted\\\"\\\\\""
    );
}
