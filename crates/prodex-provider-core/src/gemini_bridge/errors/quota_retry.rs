//! Gemini quota, retry, and rate-limit classifiers.

mod body;
mod retry_delay;

pub(super) use self::body::gemini_provider_core_plain_text_has_terminal_quota;
pub use self::retry_delay::{
    GEMINI_PROVIDER_CORE_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS,
    gemini_provider_core_invalid_stream_retry_delay_ms, gemini_provider_core_retry_delay_ms,
    gemini_provider_core_retry_delay_ms_from_body,
    gemini_provider_core_should_inline_rate_limit_retry,
};

use self::body::{
    gemini_provider_core_google_quota_message_from_value, gemini_provider_core_values_from_body,
};
use self::retry_delay::{
    gemini_provider_core_google_rate_limit_code, gemini_provider_core_google_terminal_quota_code,
    gemini_provider_core_object_mentions_quota_limit,
};

pub fn gemini_provider_core_response_retryable_quota(status: u16) -> bool {
    matches!(status, 403 | 429)
}

pub fn gemini_provider_core_body_has_terminal_quota(body: &[u8]) -> bool {
    gemini_provider_core_values_from_body(body)
        .iter()
        .any(gemini_provider_core_value_has_terminal_quota)
}

pub fn gemini_provider_core_google_quota_message(body: &[u8]) -> Option<String> {
    gemini_provider_core_values_from_body(body)
        .iter()
        .find_map(gemini_provider_core_google_quota_message_from_value)
}

pub fn gemini_provider_core_should_rotate_after_quota_response(
    status: u16,
    quota_blocked: bool,
    hard_affinity: bool,
    quota_fallback_allowed: bool,
    attempt_index: usize,
    attempt_count: usize,
) -> bool {
    quota_blocked
        && gemini_provider_core_response_retryable_quota(status)
        && (!hard_affinity || quota_fallback_allowed)
        && attempt_index + 1 < attempt_count
}

pub(super) fn gemini_provider_core_value_has_terminal_quota(value: &serde_json::Value) -> bool {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(gemini_provider_core_google_terminal_quota_code)
                }) {
                    return true;
                }
                if gemini_provider_core_object_mentions_quota_limit(object, "PerDay")
                    || gemini_provider_core_object_mentions_quota_limit(object, "Daily")
                {
                    return true;
                }
                stack.extend(object.values());
            }
            serde_json::Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
    false
}

pub(super) fn gemini_provider_core_value_has_rate_limit(value: &serde_json::Value) -> bool {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(gemini_provider_core_google_rate_limit_code)
                }) {
                    return true;
                }
                stack.extend(object.values());
            }
            serde_json::Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
    false
}
