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

use self::quota_retry::{
    gemini_provider_core_value_has_rate_limit, gemini_provider_core_value_has_terminal_quota,
};
use crate::mojo_json::Document;

pub fn gemini_provider_core_normalized_error_body(status: u16, body: &[u8]) -> Option<Vec<u8>> {
    if status < 400 {
        return None;
    }
    if status == 429 {
        let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
        if !gemini_provider_core_openai_error_kind_plan(status, &value, body).admissible {
            return None;
        }
    }
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        let error = value.get("error").unwrap_or(&value);
        if error.is_object() {
            let message = gemini_provider_core_error_message(error)
                .unwrap_or_else(|| format!("Gemini upstream returned HTTP {status}."));
            let (error_type, code) = gemini_provider_core_openai_error_kind(status, &value, body);
            let normalized = serde_json::json!({
                "error": {
                    "message": message,
                    "type": error_type,
                    "param": serde_json::Value::Null,
                    "code": code,
                    "gemini_error": error,
                }
            });
            return serde_json::to_vec(&normalized).ok();
        }
    }

    let message = std::str::from_utf8(body)
        .ok()
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Gemini upstream returned HTTP {status}."));
    let gemini_error = serde_json::json!({
        "message": message.clone(),
        "status": status,
    });
    let (error_type, code) = gemini_provider_core_openai_error_kind(status, &gemini_error, body);
    let normalized = serde_json::json!({
        "error": {
            "message": message,
            "type": error_type,
            "param": serde_json::Value::Null,
            "code": code,
            "gemini_error": gemini_error,
        }
    });
    serde_json::to_vec(&normalized).ok()
}

fn gemini_provider_core_error_message(value: &serde_json::Value) -> Option<String> {
    let mut document = Document::default();
    document.push(value, None, "");
    let raw = std::str::from_utf8(&document.raw).ok()?;
    let output = prodex_mojo_core::json::provider_error_message_json(&document.nodes, raw)
        .expect("Mojo provider error message extraction returned invalid output")?;
    serde_json::from_slice(&output).ok()
}

fn gemini_provider_core_openai_error_kind_plan(
    status: u16,
    value: &serde_json::Value,
    body: &[u8],
) -> prodex_mojo_core::json::GeminiQuotaHttpErrorKind {
    prodex_mojo_core::json::gemini_quota_http_error_kind(
        status,
        gemini_provider_core_body_has_terminal_quota(body),
        gemini_provider_core_value_has_terminal_quota(value),
        gemini_provider_core_value_has_rate_limit(value),
    )
    .expect("compiled Mojo Gemini error kind mapping returned invalid output")
}

fn gemini_provider_core_openai_error_kind(
    status: u16,
    value: &serde_json::Value,
    body: &[u8],
) -> (String, String) {
    let plan = gemini_provider_core_openai_error_kind_plan(status, value, body);
    (plan.error_type, plan.error_code)
}

#[cfg(test)]
mod tests {
    use super::gemini_provider_core_normalized_error_body;

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
