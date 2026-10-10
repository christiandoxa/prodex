use super::{RuntimeUsageLimitInputFormat, runtime_session_usage_limit_marker};

fn json_marker(input: &str) -> bool {
    runtime_session_usage_limit_marker(input, RuntimeUsageLimitInputFormat::Json).unwrap()
}

#[test]
fn session_usage_limit_marker_requires_error_context() {
    assert!(runtime_session_usage_limit_marker(
            "\u{2003}You've hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at 5:08 PM.\u{3000}",
            RuntimeUsageLimitInputFormat::PlainText,
        )
        .unwrap());
    assert!(
        !runtime_session_usage_limit_marker(
            "usage limit reached",
            RuntimeUsageLimitInputFormat::PlainText,
        )
        .unwrap()
    );

    for input in [
        r#"{"type":"error","error":{"code":"RESOURCE_EXHAUSTED"}}"#,
        r#"{"type":"response.failed","error":{"code":"usage_limit_reached"}}"#,
        r#"{"type":"error","payload":{"message":"You've hit your usage limit. Try again later."}}"#,
        r#"{"type":"event_msg","payload":{"type":"error","error":{"type":"usage_not_included"}}}"#,
        r#"{"type":"event_msg","payload":{"type":"error","message":"Quota unavailable","codex_error_info":"usage_limit_exceeded"}}"#,
        r#"{"error":{"code":"insufficient_quota"}}"#,
        r#"{"type":"event_msg","payload":{"type":"error","message":"Your workspace is out of credits. Retry later."}}"#,
        r#"{"type":"event_msg","payload":{"message":"You've hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at 5:08 PM."}}"#,
    ] {
        assert!(json_marker(input), "{input}");
    }
}

#[test]
fn session_usage_limit_marker_ignores_conversation_and_non_error_text() {
    for input in [
        r#"{"messages":[{"role":"user","code":"usage_limit_reached","message":"You've hit your usage limit"}]}"#,
        r#"{"type":"event_msg","payload":{"type":"user_message","code":"usage_limit_reached","message":"You've hit your usage limit"}}"#,
        r#"{"type":"event_msg","payload":{"type":"model_reroute","message":"You've hit your usage limit"}}"#,
        r#"{"type":"event_msg","payload":{"message":"You've hit your usage limit; details follow"}}"#,
        r#"{"error":{"message":"the docs say usage_limit_reached"}}"#,
        r#"{"error":{"content":{"code":"usage_limit_reached"}}}"#,
    ] {
        assert!(!json_marker(input), "{input}");
    }
}

#[test]
fn session_usage_limit_marker_preserves_the_2048_node_scan_limit() {
    for (empty_objects, expected) in [(2_045, true), (2_046, false)] {
        let mut input = String::from(r#"{"error":["#);
        for _ in 0..empty_objects {
            input.push_str("{},");
        }
        input.push_str(r#"{"code":"usage_limit_reached"}]}"#);
        assert_eq!(json_marker(&input), expected);
    }
}
