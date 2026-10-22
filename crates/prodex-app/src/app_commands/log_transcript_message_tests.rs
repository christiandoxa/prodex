use super::transcript_events_from_session_line;
use serde_json::{Value, json};

fn show(record: &str, payload: Value) -> Vec<(String, String)> {
    transcript_events_from_session_line(&json!({"type":record,"payload":payload}).to_string())
        .into_iter()
        .map(|event| (event.source, event.text))
        .collect()
}

#[test]
fn response_messages_keep_part_priority_normalization_and_lf_order() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }
    assert_eq!(
        show(
            "response_item",
            json!({"type":"message","role":"user","content":[
                {"text":" first "},{"text":null,"content":"ignored"},{"content":"second"},
                {"text":""},{"text":"before\rafter"},{"text":"middle\0end"}
            ]})
        ),
        vec![(
            "user".into(),
            " first\nsecond\nbefore\rafter\nmiddle end".into()
        )]
    );
    let attachment = "/home/test-user/.prodex/profiles/.prodex-overlay-123/attachments/a.png";
    assert_eq!(
        show(
            "response_item",
            json!({"type":"message","role":"user","content":[
                {"text":format!("before\n\n{attachment}\nafter")},{"content":"last"}
            ]})
        ),
        vec![("user".into(), "before\n\nafter\nlast".into())]
    );
}

#[test]
fn response_message_visibility_keeps_metadata_and_raw_role_contracts() {
    for (kinds, visible) in [
        (json!(["user.text"]), true),
        (json!([]), false),
        (json!(["user.text", 1]), false),
        (json!(["internal"]), false),
        (json!(null), true),
        (json!("internal"), true),
    ] {
        let mut payload = json!({"type":"message","role":"user","content":[{"text":"visible"}],"internal_chat_message_metadata_passthrough":{"content_item_kinds":kinds}});
        assert_eq!(
            show("response_item", payload.clone()),
            if visible {
                vec![("user".into(), "visible".into())]
            } else {
                vec![]
            }
        );
        payload["role"] = json!("assistant");
        assert_eq!(
            show("response_item", payload.clone()),
            vec![("assistant".into(), "visible".into())]
        );
        payload["role"] = json!(null);
        assert_eq!(
            show("response_item", payload.clone()),
            vec![("message".into(), "visible".into())]
        );
        payload["role"] = json!("");
        assert_eq!(
            show("response_item", payload),
            vec![("".into(), "visible".into())]
        );
    }
}

#[test]
fn response_messages_ignore_invalid_content_without_using_blocked_fallbacks() {
    for content in [
        json!(null),
        json!(false),
        json!({"text":"ignored"}),
        json!(["ignored"]),
        json!([{"text":null,"content":"ignored"}]),
    ] {
        assert_eq!(
            show("response_item", json!({"type":"message","content":content})),
            vec![]
        );
    }
    assert_eq!(
        show(
            "response_item",
            json!({"type":"message","content":[null,42,{"text":"selected"}]})
        ),
        vec![("message".into(), "selected".into())]
    );
}

#[test]
fn event_messages_keep_sources_text_fields_and_full_visible_payload() {
    for (kind, source, field) in [
        ("user_message", "user", "message"),
        ("agent_message", "assistant", "message"),
        ("agent_reasoning", "reasoning", "text"),
    ] {
        let mut payload = json!({"type":kind,"message":"ignored","text":"ignored"});
        payload[field] = json!("  readable text\nnext line");
        assert_eq!(
            show("event_msg", payload.clone()),
            vec![(source.into(), "  readable text\nnext line".into())]
        );
        payload[field] = json!(false);
        assert_eq!(show("event_msg", payload), vec![]);
    }
}

#[test]
fn message_projection_keeps_unicode_redaction_and_final_text_bound() {
    assert_eq!(
        show(
            "response_item",
            json!({"type":"message","role":"user","content":[{"text":"Bearer FAKE"}]})
        ),
        vec![("user".into(), "Bearer <redacted>".into())]
    );
    let long = "é".repeat(40_000);
    for payload in [
        json!({"type":"message","role":"user","content":[{"text":long}]}),
        json!({"type":"message","role":"assistant","content":[{"text":long}]}),
    ] {
        let events = show("response_item", payload);
        assert_eq!(events.len(), 1);
        assert!(events[0].1.len() > 60_000 && events[0].1.len() <= 65_536);
        assert!(events[0].1.ends_with(" …[truncated]"));
    }
}
