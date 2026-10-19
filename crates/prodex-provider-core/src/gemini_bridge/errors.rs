//! Gemini normalized error helpers.

mod quota_retry;

pub use self::quota_retry::{
    gemini_provider_core_body_has_terminal_quota, gemini_provider_core_google_quota_message,
    gemini_provider_core_inline_retry_threshold_ms,
    gemini_provider_core_invalid_stream_retry_delay_ms,
    gemini_provider_core_response_retryable_quota, gemini_provider_core_retry_delay_ms,
    gemini_provider_core_retry_delay_ms_from_body,
    gemini_provider_core_should_inline_rate_limit_retry,
    gemini_provider_core_should_rotate_after_quota_response,
};

use self::quota_retry::gemini_provider_core_quota_body_plan;
use crate::mojo_json::Document;

pub fn gemini_provider_core_normalized_error_body(status: u16, body: &[u8]) -> Option<Vec<u8>> {
    if !prodex_mojo_core::json::gemini_error_normalization_needs_body(status)
        .expect("Mojo Gemini normalization eligibility returned invalid output")
    {
        return None;
    }
    let value = serde_json::from_slice::<serde_json::Value>(body).ok();
    let quota = value.as_ref().map(gemini_provider_core_quota_body_plan);
    let body_terminal = quota.as_ref().map_or_else(
        || gemini_provider_core_body_has_terminal_quota(body),
        |plan| plan.terminal_quota,
    );
    let context = serde_json::json!({
        "parsed": value.is_some(),
        "value": value,
        "source": std::str::from_utf8(body).ok(),
        "terminal": quota.as_ref().is_some_and(|plan| plan.terminal_quota),
        "rate": quota.as_ref().is_some_and(|plan| plan.rate_limit),
    });
    let mut document = Document::default();
    document.push(&context, None, "");
    let raw = std::str::from_utf8(&document.raw).expect("Serde error context must remain UTF-8");
    prodex_mojo_core::json::gemini_normalized_error_json(
        status,
        body_terminal,
        &document.nodes,
        raw,
    )
    .expect("Mojo Gemini normalized error returned invalid output")
}

#[cfg(test)]
mod tests {
    use super::gemini_provider_core_normalized_error_body;

    #[test]
    fn compiled_normalization_preserves_complete_wire_bytes_and_error_subtree_boundaries() {
        use serde_json::json;
        for (status, body, message, original, code, kind) in [
            (
                400,
                r#"{"error":{},"message":"outside the selected error"}"#,
                "Gemini upstream returned HTTP 400.",
                json!({}),
                "bad_request",
                "invalid_request_error",
            ),
            (
                500,
                r#"{"error":{"nested":{"message":"  inner 🧭  "}},"message":"outside"}"#,
                "inner 🧭",
                json!({"nested":{"message":"  inner 🧭  "}}),
                "provider_error",
                "server_error",
            ),
            (
                400,
                r#"{"error":null,"message":"outer"}"#,
                r#"{"error":null,"message":"outer"}"#,
                json!({"message":r#"{"error":null,"message":"outer"}"#, "status":400}),
                "bad_request",
                "invalid_request_error",
            ),
            (
                429,
                r#"[{"code":"rate_limit_exceeded"}]"#,
                r#"[{"code":"rate_limit_exceeded"}]"#,
                json!({"message":r#"[{"code":"rate_limit_exceeded"}]"#, "status":429}),
                "provider_error",
                "api_error",
            ),
            (
                500,
                "\u{2003}plain\0failure\u{3000}",
                "plain\0failure",
                json!({"message":"plain\0failure", "status":500}),
                "provider_error",
                "server_error",
            ),
            (
                500,
                " \r\n ",
                "Gemini upstream returned HTTP 500.",
                json!({"message":"Gemini upstream returned HTTP 500.", "status":500}),
                "provider_error",
                "server_error",
            ),
        ] {
            let expected = json!({"error":{"code":code, "gemini_error":original, "message":message, "param":null, "type":kind}});
            assert_eq!(
                gemini_provider_core_normalized_error_body(status, body.as_bytes()),
                Some(serde_json::to_vec(&expected).unwrap()),
                "{status}/{body:?}"
            );
        }
        let actual = gemini_provider_core_normalized_error_body(500, &[0xff, 0]).unwrap();
        let actual: serde_json::Value = serde_json::from_slice(&actual).unwrap();
        assert_eq!(
            actual,
            json!({"error":{"code":"provider_error", "gemini_error":{"message":"Gemini upstream returned HTTP 500.","status":500}, "message":"Gemini upstream returned HTTP 500.", "param":null, "type":"server_error"}})
        );
    }

    #[test]
    fn compiled_normalization_preserves_large_unicode_messages_without_a_new_body_cap() {
        let message = "x".repeat(65_537) + "🧭\0";
        let original = serde_json::json!({"message":format!(" \u{2003}{message}\u{3000}")});
        let body = serde_json::to_vec(&serde_json::json!({"error":original})).unwrap();
        let expected = serde_json::json!({"error":{"code":"provider_error","gemini_error":original,"message":message,"param":null,"type":"server_error"}});
        assert_eq!(
            gemini_provider_core_normalized_error_body(500, &body),
            Some(serde_json::to_vec(&expected).unwrap())
        );
    }

    #[test]
    fn normalized_error_body_uses_mojo_message_precedence_for_nested_errors() {
        let body = br#"{"error":{"detail":"nested detail","message":"inner message"},"message":"top message"}"#;
        let normalized = gemini_provider_core_normalized_error_body(400, body)
            .expect("structured Gemini errors should normalize");
        let value: serde_json::Value = serde_json::from_slice(&normalized).unwrap();
        assert_eq!(value["error"]["message"], "inner message");

        let nested = br#"{"error":{"detail":" nested detail "}}"#;
        let normalized = gemini_provider_core_normalized_error_body(400, nested)
            .expect("nested Gemini errors should normalize");
        let value: serde_json::Value = serde_json::from_slice(&normalized).unwrap();
        assert_eq!(value["error"]["message"], "nested detail");

        let direct =
            br#"{"error":{"detail":"direct detail","nested":{"message":"nested message"}}}"#;
        let normalized = gemini_provider_core_normalized_error_body(400, direct)
            .expect("direct nested Gemini errors should normalize");
        let value: serde_json::Value = serde_json::from_slice(&normalized).unwrap();
        assert_eq!(value["error"]["message"], "direct detail");
    }

    #[test]
    fn compiled_gemini_http_error_mapping_preserves_quota_priority_and_generic_passthrough() {
        for (status, body, error_type, code) in [
            (
                400,
                "{\"error\":{\"message\":\"bad 🧭\"}}",
                "invalid_request_error",
                "bad_request",
            ),
            (
                401,
                "authentication failed",
                "authentication_error",
                "invalid_authentication",
            ),
            (
                403,
                "permission denied",
                "permission_error",
                "permission_denied",
            ),
            (404, "not found", "invalid_request_error", "not_found"),
            (500, "server failure", "server_error", "provider_error"),
            (599, "server failure", "server_error", "provider_error"),
            (600, "other failure", "api_error", "provider_error"),
            (
                429,
                "{\"error\":{\"code\":\"RATE_LIMIT_EXCEEDED\",\"message\":\"wait\"}}",
                "rate_limit_error",
                "rate_limit_exceeded",
            ),
            (
                403,
                "{\"error\":{\"status\":\"RESOURCE_EXHAUSTED\"}}",
                "insufficient_quota",
                "insufficient_quota",
            ),
            (
                429,
                "{\"error\":{\"status\":\"RESOURCE_EXHAUSTED\",\"code\":\"RATE_LIMIT_EXCEEDED\"}}",
                "insufficient_quota",
                "insufficient_quota",
            ),
        ] {
            let bytes =
                gemini_provider_core_normalized_error_body(status, body.as_bytes()).unwrap();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["error"]["type"], error_type, "{status}/{body}");
            assert_eq!(value["error"]["code"], code, "{status}/{body}");
        }
        for body in [
            b"Too Many Requests".as_slice(),
            br#"{"error":{"message":"Too Many Requests"}}"#,
            br#"{"error":{"code":429}}"#,
            br#"{"error":{"status":"RESOURCE_EXHAUSTED"}"#,
            b"data: {\"error\":{\"status\":\"RESOURCE_EXHAUSTED\"}}\n",
            &[0xff, 0x00],
        ] {
            assert_eq!(gemini_provider_core_normalized_error_body(429, body), None);
        }
        assert_eq!(gemini_provider_core_normalized_error_body(200, b"{}"), None);
    }
}
