#[test]
fn quota_reset_at_from_message_parses_ordinal_date() {
    let parsed = quota_reset_at_from_message(
        "You've hit your usage limit. Try again at Mar 24th, 2026 2:04 AM.",
    )
    .expect("reset timestamp should parse");
    let expected = Local
        .with_ymd_and_hms(2026, 3, 24, 2, 4, 0)
        .single()
        .or_else(|| Local.with_ymd_and_hms(2026, 3, 24, 2, 4, 0).earliest())
        .expect("local timestamp should resolve")
        .timestamp();

    assert_eq!(parsed, expected);
}

#[test]
fn quota_reset_at_from_message_parses_usage_limit_json() {
    let parsed = quota_reset_at_from_message(
        r#"{"type":"error","error":{"type":"usage_limit_reached","message":"The usage limit has been reached","resets_at":1780245646},"headers":{"X-Codex-Primary-Reset-At":"1780241111"}}"#,
    )
    .expect("reset timestamp should parse from usage limit JSON");

    assert_eq!(parsed, 1_780_245_646);
}
#[test]
fn quota_reset_at_from_message_parses_codex_reset_headers() {
    let parsed = quota_reset_at_from_message(
        r#"{"status_code":429,"headers":{"X-Codex-Primary-Used-Percent":"100","X-Codex-Primary-Reset-At":"1780245646","X-Codex-Secondary-Used-Percent":"91","X-Codex-Secondary-Reset-At":"1780243339"}}"#,
    )
    .expect("reset timestamp should parse from Codex reset headers");

    assert_eq!(parsed, 1_780_245_646);
}

