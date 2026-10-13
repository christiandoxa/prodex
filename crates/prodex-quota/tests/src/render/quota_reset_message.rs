use super::*;

#[test]
fn quota_reset_json_uses_top_level_then_nested_candidate_order() {
    let cases = [
        (
            r#"{"resets_at":1,"reset_at":2,"error":{"resets_at":3,"reset_at":4}}"#,
            Some(1),
        ),
        (
            r#"{"resets_at":"bad","reset_at":-2,"error":{"resets_at":3,"reset_at":4}}"#,
            Some(-2),
        ),
        (
            r#"{"resets_at":false,"reset_at":{},"error":{"resets_at":" -3 ","reset_at":4}}"#,
            Some(-3),
        ),
        (
            r#"{"resets_at":null,"reset_at":false,"error":{"resets_at":0,"reset_at":4}}"#,
            Some(0),
        ),
        (r#"{"headers":{"X-Codex-Primary-Reset-At":5}}"#, Some(5)),
    ];

    for (message, expected) in cases {
        assert_eq!(quota_reset_at_from_message(message), expected, "{message}");
    }
}

#[test]
fn quota_reset_json_uses_used_percent_gates_and_header_fallback_order() {
    let cases = [
        (
            r#"{"headers":{"X-Codex-Primary-Used-Percent":"100","X-Codex-Primary-Reset-At":"-10","X-Codex-Secondary-Used-Percent":"100","X-Codex-Secondary-Reset-At":"20"}}"#,
            Some(-10),
        ),
        (
            r#"{"headers":{"X-Codex-Primary-Used-Percent":99,"X-Codex-Primary-Reset-At":10,"X-Codex-Secondary-Used-Percent":100,"X-Codex-Secondary-Reset-At":20}}"#,
            Some(20),
        ),
        (
            r#"{"headers":{"X-Codex-Primary-Used-Percent":100,"X-Codex-Secondary-Reset-At":20}}"#,
            None,
        ),
        (
            r#"{"headers":{"X-Codex-Primary-Reset-At":10,"X-Codex-Secondary-Used-Percent":100}}"#,
            None,
        ),
        (
            r#"{"headers":{"X-Codex-Primary-Reset-At":" -10 ","X-Codex-Secondary-Reset-At":20}}"#,
            Some(-10),
        ),
        (
            r#"{"headers":{"x-cOdEx-pRiMaRy-uSeD-pErCeNt":"100","X-cOdEx-pRiMaRy-rEsEt-aT":"-30","x-CODEX-secondary-used-percent":"99","x-codex-secondary-reset-at":"40"}}"#,
            Some(-30),
        ),
        (
            r#"{"headers":{"x-codex-secondary-reset-at":"40"}}"#,
            Some(40),
        ),
    ];

    for (message, expected) in cases {
        assert_eq!(quota_reset_at_from_message(message), expected, "{message}");
    }
}

#[test]
fn quota_reset_json_preserves_serde_duplicate_key_behavior() {
    assert_eq!(
        quota_reset_at_from_message(r#"{"resets_at":1,"resets_at":-2}"#),
        Some(-2),
    );
    assert_eq!(
        quota_reset_at_from_message(
            r#"{"headers":{"X-Codex-Primary-Reset-At":1,"X-Codex-Primary-Reset-At":-2}}"#,
        ),
        Some(-2),
    );
}

#[test]
fn quota_reset_json_ignores_malformed_and_non_object_values() {
    for message in ["{", "[]", "null", "false", "\"value\"", "42"] {
        assert_eq!(quota_reset_at_from_message(message), None, "{message}");
    }
}

#[test]
fn human_retry_clock_is_future_local_time_without_changing_model_or_session() {
    let before = chrono::Local::now().timestamp();
    let parsed = quota_reset_at_from_message("You've hit your usage limit. Try again at 5:08 PM.")
        .expect("local clock should parse");
    let after = chrono::Local::now().timestamp();
    assert!(
        parsed > before - 120,
        "must not return a long expired retry window"
    );
    assert!(
        parsed <= after + 26 * 3_600,
        "a local clock must resolve by next day"
    );
    for malformed in [
        "Your account is limited. Try again at yesterday.",
        "Your account is limited. Try again at Mar unknown, 2026 2:04 AM.",
        "Your account is limited. Try again at 99:88 PM.",
    ] {
        assert_eq!(quota_reset_at_from_message(malformed), None, "{malformed}");
    }
}
