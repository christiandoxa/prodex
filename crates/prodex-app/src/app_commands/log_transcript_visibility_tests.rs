use super::{transcript_events_from_session_line, transcript_user_message_is_visible};
use serde_json::json;

#[test]
fn decoded_log_visibility_keeps_absent_metadata_fast_path_and_filters_internal_context() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }
    assert!(transcript_user_message_is_visible(
        &json!({"role":"assistant"})
    ));
    for (kinds, visible) in [
        (json!(["user.text"]), true),
        (json!([]), false),
        (json!(["user.text", 1]), false),
        (json!(["internal"]), false),
        (json!(null), true),
        (json!("internal"), true),
    ] {
        let payload = json!({"type":"message", "role":"user", "content":[{"type":"input_text", "text":"visible prompt"}],
            "internal_chat_message_metadata_passthrough":{"content_item_kinds":kinds}});
        assert_eq!(transcript_user_message_is_visible(&payload), visible);
        let line = json!({"type":"response_item", "payload":payload}).to_string();
        let events = transcript_events_from_session_line(&line);
        let text: Vec<_> = events.iter().map(|event| event.text.as_str()).collect();
        assert_eq!(
            text,
            if visible {
                vec!["visible prompt"]
            } else {
                vec![]
            }
        );
    }
    assert!(!transcript_user_message_is_visible(
        &json!({"role":"assistant", "internal_chat_message_metadata_passthrough":{"content_item_kinds":["user.text"]}})
    ));
}
