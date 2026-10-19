//! Provider error body token extraction.

use crate::mojo_json::Document;
use serde_json::Value;

pub(super) fn provider_error_codes(body: &[u8]) -> Vec<String> {
    crate::mojo_json::values_from_json_or_sse(body)
        .iter()
        .flat_map(provider_error_codes_from_value)
        .collect()
}

fn provider_error_codes_from_value(value: &Value) -> Vec<String> {
    let mut document = Document::default();
    document.push(value, None, "");
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    prodex_mojo_core::json::provider_error_codes_json(&document.nodes, raw)
        .ok()
        .and_then(|body| serde_json::from_slice(&body).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::provider_error_codes;

    #[test]
    fn provider_error_codes_mojo_preserves_structured_fields_without_message_leakage() {
        let cases: &[(&[u8], &[&str])] = &[
            (
                br#"{"code":" UNAVAILABLE ","message":"do not classify me"}"#,
                &["unavailable"],
            ),
            (br#"{"error":"overloaded","message":"rate limit"}"#, &[]),
            (br#"{"error":[{"code":"A"},{"code":"A"}] }"#, &["a", "a"]),
            (br#"{"code":["ignored"],"status":false,"reason":null}"#, &[]),
            (br#"{"reason":"   ","status":-429}"#, &["-429"]),
            (b"not-json", &[]),
            (b"", &[]),
        ];
        for (body, expected) in cases {
            assert_eq!(provider_error_codes(body), *expected, "body={body:?}");
        }
    }

    #[test]
    fn provider_error_codes_mojo_preserves_sse_framing_and_skips_malformed_events() {
        let body = b"event: error\r\ndata: {\"code\":\"BUSY\"}\r\n\r\ndata: {bad json}\n\ndata: {\"status\":429}\n\n";
        assert_eq!(provider_error_codes(body), ["busy", "429"]);
    }

    #[test]
    fn buffered_sse_error_codes_share_unicode_framing_without_joining_data_lines() {
        let body = "\u{2003}data:\u{3000}{\"code\":\"BUSY\"}\r\ndata: [DONE]\ndata:\nDATA: {\"code\":\"ignored\"}\ndata: {\"code\":\ndata: \"split-is-not-a-value\"}\ndata:{\"status\":429}\ndata:{\"code\":\"BUSY\"}";
        assert_eq!(
            provider_error_codes(body.as_bytes()),
            ["busy", "429", "busy"]
        );
        assert_eq!(provider_error_codes(b"data:\xff"), Vec::<String>::new());
        assert_eq!(
            provider_error_codes(br#""data: {\"code\":\"ignored\"}""#),
            Vec::<String>::new()
        );
    }

    #[test]
    fn provider_error_codes_mojo_preserves_nested_order_and_normalizes_tokens() {
        let body = br#"{"error":[{"code":"  RESOURCE_EXHAUSTED ","status":429},{"reason":"Slow_Down"}],"message":"not a code"}"#;
        assert_eq!(
            provider_error_codes(body),
            ["resource_exhausted", "429", "slow_down"]
        );
    }

    #[test]
    fn provider_error_codes_mojo_reads_sse_data_and_ignores_bare_error_text() {
        let body = b"event: error\ndata: {\"error\":{\"code\":\"INVALID_API_KEY\",\"message\":\"bad\"}}\n\n";
        assert_eq!(provider_error_codes(body), ["invalid_api_key"]);
    }

    #[test]
    fn provider_error_codes_mojo_keeps_case_sensitive_keys_unicode_and_duplicates() {
        let body = serde_json::to_vec(&serde_json::json!({
            "CODE": "ignored",
            "error": "insufficient_quota",
            "nested": [{"type": "\u{2003}THÉ_雪\u{3000}"}, {"code": "repeat"}, {"code": "repeat"}],
            "reason": " \"Quote\"\\Line\nNext ",
            "status": " \t ",
        }))
        .unwrap();
        assert_eq!(
            provider_error_codes(&body),
            ["thÉ_雪", "repeat", "repeat", "\"quote\"\\line\nnext"]
        );
        assert!(provider_error_codes(br#"{"code":["not-direct"],"status":false}"#).is_empty());
    }
}
