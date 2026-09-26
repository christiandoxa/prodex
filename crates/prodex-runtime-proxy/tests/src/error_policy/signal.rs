use super::*;

#[test]
fn error_signal_extractors_match_expected_values() {
    let json_cases = [
        (
            RuntimeHttpErrorClass::Quota,
            serde_json::json!({"error": {"code": "insufficient_quota", "message": "quota gone"}}),
            Some("quota gone"),
        ),
        (
            RuntimeHttpErrorClass::Quota,
            serde_json::json!({"outer": [{"message": "You've hit your usage limit. Try again at 10:00."}]}),
            Some("You've hit your usage limit. Try again at 10:00."),
        ),
        (
            RuntimeHttpErrorClass::RateLimited,
            serde_json::json!({"error": {"type": "rate_limit_exceeded", "detail": "slow down"}}),
            Some("slow down"),
        ),
        (
            RuntimeHttpErrorClass::ProfileUnavailable,
            serde_json::json!({"error": {"reason": "deactivated_workspace", "message": "workspace disabled"}}),
            Some("workspace disabled"),
        ),
        (
            RuntimeHttpErrorClass::Overload,
            serde_json::json!({"nested": {"message": "Selected model is at capacity. Please try again."}}),
            Some("Selected model is at capacity. Please try again."),
        ),
        (
            RuntimeHttpErrorClass::Other,
            serde_json::json!({"error": {"code": "insufficient_quota", "message": "ignored"}}),
            None,
        ),
    ];
    for (class, value, expected) in json_cases {
        assert_eq!(
            runtime_error_signal_message_from_value(&value, class).as_deref(),
            expected,
            "class={class:?} value={value}",
        );
    }

    let text_cases = [
        (
            RuntimeHttpErrorClass::Quota,
            " You've hit your usage limit. ",
            Some("You've hit your usage limit."),
        ),
        (
            RuntimeHttpErrorClass::RateLimited,
            "request failed: RATE_LIMIT_EXCEEDED",
            Some("request failed: RATE_LIMIT_EXCEEDED"),
        ),
        (
            RuntimeHttpErrorClass::ProfileUnavailable,
            "DEACTIVATED_WORKSPACE",
            Some("DEACTIVATED_WORKSPACE"),
        ),
        (
            RuntimeHttpErrorClass::Overload,
            "Selected model is at capacity; please try again.",
            Some("Selected model is at capacity; please try again."),
        ),
        (RuntimeHttpErrorClass::Other, "insufficient_quota", None),
        (
            RuntimeHttpErrorClass::TransientServer,
            "server is overloaded",
            None,
        ),
        (RuntimeHttpErrorClass::Quota, "generic quota prose", None),
    ];
    for (class, text, expected) in text_cases {
        assert_eq!(
            runtime_error_signal_message_from_text(text, class).as_deref(),
            expected,
            "class={class:?} text={text:?}",
        );
    }
}
