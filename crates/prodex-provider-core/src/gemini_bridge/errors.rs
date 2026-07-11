//! Gemini normalized error helpers.

mod quota_retry;

pub use self::quota_retry::{
    GEMINI_PROVIDER_CORE_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS,
    gemini_provider_core_body_has_terminal_quota, gemini_provider_core_google_quota_message,
    gemini_provider_core_invalid_stream_retry_delay_ms,
    gemini_provider_core_response_retryable_quota, gemini_provider_core_retry_delay_ms,
    gemini_provider_core_retry_delay_ms_from_body,
    gemini_provider_core_should_inline_rate_limit_retry,
    gemini_provider_core_should_rotate_after_quota_response,
};

use self::quota_retry::{
    gemini_provider_core_plain_text_has_terminal_quota, gemini_provider_core_value_has_rate_limit,
    gemini_provider_core_value_has_terminal_quota,
};

pub fn gemini_provider_core_normalized_error_body(status: u16, body: &[u8]) -> Option<Vec<u8>> {
    if status < 400 {
        return None;
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
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                for key in ["message", "detail", "error"] {
                    if let Some(message) = object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .filter(|message| !message.trim().is_empty())
                    {
                        return Some(message.to_string());
                    }
                }
                stack.extend(object.values());
            }
            serde_json::Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
    None
}

fn gemini_provider_core_openai_error_kind(
    status: u16,
    value: &serde_json::Value,
    body: &[u8],
) -> (&'static str, &'static str) {
    if gemini_provider_core_body_has_terminal_quota(body)
        || gemini_provider_core_value_has_terminal_quota(value)
        || gemini_provider_core_plain_text_has_terminal_quota(body)
    {
        return ("insufficient_quota", "insufficient_quota");
    }
    if status == 429 || gemini_provider_core_value_has_rate_limit(value) {
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
