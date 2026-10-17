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
        if !gemini_provider_core_value_has_terminal_quota(&value)
            && !gemini_provider_core_value_has_rate_limit(&value)
        {
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

fn gemini_provider_core_openai_error_kind(
    status: u16,
    value: &serde_json::Value,
    body: &[u8],
) -> (&'static str, &'static str) {
    if gemini_provider_core_body_has_terminal_quota(body)
        || gemini_provider_core_value_has_terminal_quota(value)
    {
        return ("insufficient_quota", "insufficient_quota");
    }
    if gemini_provider_core_value_has_rate_limit(value) {
        return ("rate_limit_error", "rate_limit_exceeded");
    }
    match status {
        400 => ("invalid_request_error", "bad_request"),
        401 => ("authentication_error", "invalid_authentication"),
        403 => ("permission_error", "permission_denied"),
        404 => ("invalid_request_error", "not_found"),
        500..=599 => ("server_error", "provider_error"),
        _ => ("api_error", "provider_error"),
    }
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
}
