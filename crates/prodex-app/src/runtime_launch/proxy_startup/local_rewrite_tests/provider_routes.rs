use super::super::local_rewrite_gemini_compact::runtime_gemini_local_compact_response_parts;

#[test]
fn translated_provider_remote_compact_uses_local_emulation() {
    let parts = runtime_gemini_local_compact_response_parts(
        br#"{"model":"test-model","input":[{"type":"message","role":"user","content":[{"type":"input_text","text":"retain this context"}]}]}"#,
    );
    let body: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();

    assert_eq!(parts.status, 200);
    assert!(
        body["output"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("retain this context")
    );
}
