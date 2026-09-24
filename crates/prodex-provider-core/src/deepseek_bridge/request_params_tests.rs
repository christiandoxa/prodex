use super::deepseek_provider_core_user_id_from_responses_request;

#[test]
fn user_id_kernel_enforces_ascii_and_byte_limit_after_rust_trim() {
    let normalized = deepseek_provider_core_user_id_from_responses_request(
        &serde_json::json!({"user_id": " \u{2003}user_1\u{00a0} "}),
        "DeepSeek",
    )
    .unwrap();
    assert_eq!(normalized.as_deref(), Some("user_1"));

    let at_limit = deepseek_provider_core_user_id_from_responses_request(
        &serde_json::json!({"user_id": "u".repeat(512)}),
        "DeepSeek",
    )
    .unwrap();
    assert_eq!(at_limit.as_deref().map(str::len), Some(512));

    let too_long = deepseek_provider_core_user_id_from_responses_request(
        &serde_json::json!({"user_id": "u".repeat(513)}),
        "DeepSeek",
    )
    .unwrap_err();
    assert!(too_long.contains("at most 512 bytes"));

    let non_ascii = deepseek_provider_core_user_id_from_responses_request(
        &serde_json::json!({"user_id": "user-é"}),
        "DeepSeek",
    )
    .unwrap_err();
    assert!(non_ascii.contains("only letters, numbers"));
}
