//! Gemini retry-delay parsing.

#[path = "retry_delay/duration.rs"]
mod duration;

use self::duration::{
    gemini_provider_core_duration_ms, gemini_provider_core_retry_after_header_ms,
    gemini_provider_core_retry_delay_ms_from_message,
};
use super::gemini_provider_core_values_from_body;

const GEMINI_PROVIDER_CORE_RETRY_AFTER_CAP_MS: u64 = 300_000;
const GEMINI_PROVIDER_CORE_RATE_LIMIT_RETRY_DELAYS_MS: &[u64] = &[
    5_000, 10_000, 20_000, 30_000, 30_000, 30_000, 30_000, 30_000, 30_000,
];
pub const GEMINI_PROVIDER_CORE_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS: u64 = 30_000;
const GEMINI_PROVIDER_CORE_INVALID_STREAM_RETRY_BASE_DELAY_MS: u64 = 1_000;

pub fn gemini_provider_core_retry_delay_ms_from_body(body: &[u8]) -> Option<u64> {
    gemini_provider_core_values_from_body(body)
        .iter()
        .filter_map(gemini_provider_core_retry_delay_ms_from_value)
        .max()
}

pub fn gemini_provider_core_retry_delay_ms(
    retry_after: Option<&str>,
    body: &[u8],
    retry_index: usize,
) -> Option<u64> {
    let default_ms = *GEMINI_PROVIDER_CORE_RATE_LIMIT_RETRY_DELAYS_MS.get(retry_index)?;
    let server_ms = [
        retry_after.and_then(gemini_provider_core_retry_after_header_ms),
        gemini_provider_core_retry_delay_ms_from_body(body),
    ]
    .into_iter()
    .flatten()
    .max();
    Some(
        server_ms
            .map(|delay_ms| delay_ms.max(default_ms))
            .unwrap_or(default_ms)
            .min(GEMINI_PROVIDER_CORE_RETRY_AFTER_CAP_MS),
    )
}

pub fn gemini_provider_core_should_inline_rate_limit_retry(delay_ms: u64) -> bool {
    delay_ms > 0 && delay_ms <= GEMINI_PROVIDER_CORE_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS
}

pub fn gemini_provider_core_invalid_stream_retry_delay_ms(retry_index: usize) -> u64 {
    GEMINI_PROVIDER_CORE_INVALID_STREAM_RETRY_BASE_DELAY_MS
        .saturating_mul(1_u64 << retry_index.min(8))
}

fn gemini_provider_core_retry_delay_ms_from_value(value: &serde_json::Value) -> Option<u64> {
    let mut best = None;
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(delay_ms) = object
                    .get("retryDelay")
                    .and_then(serde_json::Value::as_str)
                    .and_then(gemini_provider_core_duration_ms)
                {
                    gemini_provider_core_update_max_ms(&mut best, delay_ms);
                }
                for key in ["message", "detail", "error"] {
                    if let Some(delay_ms) = object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .and_then(gemini_provider_core_retry_delay_ms_from_message)
                    {
                        gemini_provider_core_update_max_ms(&mut best, delay_ms);
                    }
                }
                if ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(gemini_provider_core_google_rate_limit_code)
                }) {
                    gemini_provider_core_update_max_ms(&mut best, 10_000);
                }
                if gemini_provider_core_object_mentions_quota_limit(object, "PerMinute") {
                    gemini_provider_core_update_max_ms(&mut best, 60_000);
                }
                stack.extend(object.values());
            }
            serde_json::Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
    best
}

fn gemini_provider_core_update_max_ms(best: &mut Option<u64>, candidate: u64) {
    if candidate == 0 {
        return;
    }
    *best = Some(best.map_or(candidate, |current| current.max(candidate)));
}

pub(super) fn gemini_provider_core_object_mentions_quota_limit(
    object: &serde_json::Map<String, serde_json::Value>,
    needle: &str,
) -> bool {
    object
        .get("quotaId")
        .or_else(|| object.get("quota_limit"))
        .or_else(|| object.get("quotaLimit"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|quota| quota.contains(needle))
        || object
            .get("metadata")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|metadata| {
                gemini_provider_core_object_mentions_quota_limit(metadata, needle)
            })
}

pub(super) fn gemini_provider_core_google_rate_limit_code(code: &str) -> bool {
    matches!(
        code.trim().to_ascii_lowercase().as_str(),
        "rate_limit_exceeded" | "rate_limit_exceeded_error"
    )
}

pub(super) fn gemini_provider_core_google_terminal_quota_code(code: &str) -> bool {
    matches!(
        code.trim().to_ascii_lowercase().as_str(),
        "quota_exhausted" | "insufficient_g1_credits_balance" | "insufficient_quota"
    )
}
