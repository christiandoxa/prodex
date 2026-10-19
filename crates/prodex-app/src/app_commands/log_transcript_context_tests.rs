use super::super::{TranscriptEvent, local_log_timestamp, transcript_events_from_session_line};
use serde_json::{Value, json};

fn records(kind: &str, payload: Value) -> Vec<TranscriptEvent> {
    transcript_events_from_session_line(
        &json!({
            "timestamp": "2026-01-01T00:00:00Z", "type": kind, "payload": payload,
        })
        .to_string(),
    )
}

fn event(source: &str, text: &str) -> TranscriptEvent {
    TranscriptEvent {
        timestamp: local_log_timestamp("2026-01-01T00:00:00Z"),
        source: source.to_string(),
        text: text.to_string(),
    }
}

#[test]
fn context_projection_preserves_provider_presence_type_and_empty_string_rules() {
    for primary in [Value::Null, json!(true), json!(42), json!([]), json!({})] {
        assert!(
            records(
                "session_meta",
                json!({"model_provider": primary, "provider": "fallback"})
            )
            .is_empty()
        );
    }
    assert_eq!(
        records("session_meta", json!({"provider": "fallback"})),
        vec![event("session-context", "provider=fallback")]
    );
    assert_eq!(
        records(
            "session_meta",
            json!({"model_provider": "", "provider": "fallback"})
        ),
        vec![event("session-context", "provider=")]
    );
    for payload in [
        Value::Null,
        json!([]),
        json!("text"),
        json!({"base_instructions": {"text": " \u{2003}\r\n"}}),
    ] {
        assert!(records("session_meta", payload.clone()).is_empty());
        assert!(records("turn_context", payload).is_empty());
    }
}

#[test]
fn context_projection_preserves_field_order_raw_whitespace_and_final_redaction() {
    assert_eq!(
        records(
            "session_meta",
            json!({
                "base_instructions": {"text": " \u{2003}guide\nBearer FAKE\0"},
                "model_provider": "primary", "provider": "ignored", "source": "",
                "originator": "tool", "cwd": "/home/test-user/project",
            })
        ),
        vec![
            event("prompt-engineering", " \u{2003}guide\nBearer <redacted>"),
            event(
                "session-context",
                "provider=primary source= originator=tool cwd=/home/test-user/project"
            ),
        ]
    );
    assert_eq!(
        records(
            "turn_context",
            json!({
                "cwd": "\u{3000}東京🧭\n", "approval_policy": "never", "summary": "",
                "effort": "high", "model": "mock-model", "unknown": "ignored",
            })
        ),
        vec![event(
            "turn-context",
            "model=mock-model effort=high summary= approval=never cwd=\u{3000}東京🧭\n"
        )]
    );
}

#[test]
fn reasoning_projection_preserves_fallback_precedence_shape_and_part_order() {
    assert_eq!(
        records(
            "response_item",
            json!({"type": "reasoning", "summary": [
                " one ", {"text": "two", "summary_text": "ignored"},
                {"summary_text": "三🧭"}, {"text": null, "summary_text": "blocked"},
                {"text": "\u{3000}", "summary_text": "blocked"}, null, 7, true, [],
            ]})
        ),
        vec![event("reasoning", " one \ntwo\n三🧭")]
    );
    for summary in [
        json!("\u{2003}"),
        json!([]),
        json!({"text": false, "summary_text": "blocked"}),
        Value::Null,
    ] {
        assert!(
            records(
                "response_item",
                json!({"type": "reasoning", "summary": summary})
            )
            .is_empty()
        );
    }
    for summary in [
        json!("single"),
        json!({"text": "single"}),
        json!({"summary_text": "single"}),
    ] {
        assert_eq!(
            records(
                "response_item",
                json!({"type": "reasoning", "summary": summary})
            ),
            vec![event("reasoning", "single")]
        );
    }
}

#[test]
fn context_projection_reaches_existing_final_text_bound_after_large_unicode_inputs() {
    let text = "東京🧭\0\n".repeat(8192);
    let payload = json!({"base_instructions": {"text": text}});
    let tree = crate::json_boundary::app_json_tree_without_wire(&payload).unwrap();
    let wire = prodex_mojo_core::json::log_transcript_context(
        &tree.nodes,
        prodex_mojo_core::json::TranscriptContextKind::SessionMetadata,
    )
    .unwrap();
    let projected: Vec<(String, String)> = serde_json::from_slice(&wire).unwrap();
    assert_eq!(
        projected,
        vec![("prompt-engineering".to_string(), text.clone())]
    );
    let final_text = prodex_mojo_core::log::normalize_transcript_safe_text(&text).unwrap();
    assert_eq!(
        records("session_meta", payload),
        vec![event("prompt-engineering", &final_text)]
    );
    assert!(final_text.len() <= 65_536);
    assert!(final_text.ends_with(" …[truncated]"));
}
