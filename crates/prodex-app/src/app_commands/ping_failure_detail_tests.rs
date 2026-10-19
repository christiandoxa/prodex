use super::super::ping_structured_failure_detail;
use serde_json::json;

#[test]
fn structured_ping_failure_uses_first_error_and_preserves_field_order() {
    let first = json!({"type": "turn.failed", "message": " top ", "code": " C ",
        "error": {"message": "inner", "code": "I"}});
    let text = format!(
        "invalid\nnull\n[]\n{{\"type\":\"other\"}}\n{first}\n{{\"type\":\"error\",\"message\":\"later\"}}"
    );
    assert_eq!(
        ping_structured_failure_detail(&text),
        Some("top   C  inner I".to_string())
    );
    let text = json!({"type": "error", "message": "message", "code": "code", "error": "simple"})
        .to_string();
    assert_eq!(
        ping_structured_failure_detail(&text),
        Some("message code simple".to_string())
    );
}

#[test]
fn structured_ping_failure_keeps_fallback_shape_and_serde_escape_rules() {
    for (text, expected) in [
        (r#"{"type":"turn.\u0066ailed"}"#, Some("turn failed")),
        (
            r#"{"type":"error","message":false,"code":[],"error":{"message":null,"code":"\u3000"}}"#,
            Some("Codex error"),
        ),
        ("", None),
        ("invalid\n{}\n[]\nnull", None),
        (r#"{"type":"TURN.FAILED","message":"ignored"}"#, None),
    ] {
        assert_eq!(ping_structured_failure_detail(text).as_deref(), expected);
    }
}

#[test]
fn structured_ping_failure_redacts_joined_fields_before_normalization_and_byte_limits() {
    let text = json!({"type": "error", "message": "Bearer", "code": "FAKE"}).to_string();
    assert_eq!(
        ping_structured_failure_detail(&text),
        Some("Bearer <redacted>".to_string())
    );
    let text = json!({"type": "error", "message": "\u{2003}left\r\nright\0\u{3000}"}).to_string();
    assert_eq!(
        ping_structured_failure_detail(&text),
        Some("left  right\0".to_string())
    );
    let text = json!({"type": "error", "message": "x".repeat(4095) + "🧭tail", "code": "code"})
        .to_string();
    assert_eq!(
        ping_structured_failure_detail(&text),
        Some("x".repeat(4095))
    );
}
