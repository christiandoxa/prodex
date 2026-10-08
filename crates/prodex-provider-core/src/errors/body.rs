//! Provider error body token extraction.

use crate::mojo_json::Document;
use serde_json::Value;

pub(super) fn provider_error_codes(body: &[u8]) -> Vec<String> {
    if let Ok(value) = serde_json::from_slice::<Value>(body) {
        return provider_error_codes_from_value(&value);
    }
    std::str::from_utf8(body)
        .ok()
        .into_iter()
        .flat_map(|text| text.lines())
        .filter_map(|line| line.trim().strip_prefix("data:").map(str::trim))
        .filter_map(|payload| serde_json::from_str::<Value>(payload).ok())
        .flat_map(|value| provider_error_codes_from_value(&value))
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
