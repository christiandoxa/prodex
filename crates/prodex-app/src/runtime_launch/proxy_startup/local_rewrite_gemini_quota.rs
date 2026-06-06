use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use runtime_proxy_crate::extract_runtime_proxy_quota_message;

const RUNTIME_GEMINI_RETRY_AFTER_CAP_MS: u64 = 300_000;
const RUNTIME_GEMINI_CLOUD_CODE_DEFAULT_RETRY_MS: u64 = 10_000;
const RUNTIME_GEMINI_PER_MINUTE_RETRY_MS: u64 = 60_000;
const RUNTIME_GEMINI_RATE_LIMIT_RETRY_DELAYS_MS: &[u64] = &[
    5_000, 10_000, 20_000, 30_000, 30_000, 30_000, 30_000, 30_000, 30_000,
];

pub(super) fn runtime_gemini_buffered_parts_are_quota_blocked(
    status: u16,
    parts: &RuntimeHeapTrimmedBufferedResponseParts,
) -> bool {
    runtime_gemini_response_retryable_quota(status)
        && (extract_runtime_proxy_quota_message(&parts.body).is_some()
            || runtime_gemini_google_quota_message(&parts.body).is_some())
        || runtime_provider_error_class(RuntimeProviderBridgeKind::Gemini, status, &parts.body)
            == RuntimeProviderErrorClass::Quota
}

pub(super) fn runtime_gemini_retry_delay_ms(
    retry_after: Option<&str>,
    body: &[u8],
    retry_index: usize,
) -> Option<u64> {
    let default_ms = *RUNTIME_GEMINI_RATE_LIMIT_RETRY_DELAYS_MS.get(retry_index)?;
    let server_ms = [
        retry_after.and_then(runtime_gemini_retry_after_header_ms),
        runtime_gemini_retry_delay_ms_from_body(body),
    ]
    .into_iter()
    .flatten()
    .max();
    Some(
        server_ms
            .map(|delay_ms| delay_ms.max(default_ms))
            .unwrap_or(default_ms)
            .min(RUNTIME_GEMINI_RETRY_AFTER_CAP_MS),
    )
}

pub(super) fn runtime_gemini_body_has_terminal_quota(body: &[u8]) -> bool {
    runtime_gemini_values_from_body(body)
        .iter()
        .any(runtime_gemini_value_has_terminal_quota)
}

pub(super) fn runtime_gemini_response_retryable_quota(status: u16) -> bool {
    matches!(status, 403 | 429)
}

pub(super) fn runtime_gemini_normalized_error_parts(
    status: u16,
    mut parts: RuntimeHeapTrimmedBufferedResponseParts,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let Some(body) = runtime_gemini_normalized_error_body(status, &parts.body) else {
        return parts;
    };
    parts.headers.retain(|(name, _)| {
        !matches!(
            name.to_ascii_lowercase().as_str(),
            "content-length" | "content-type"
        )
    });
    parts.headers.push((
        "content-type".to_string(),
        b"application/json; charset=utf-8".to_vec(),
    ));
    parts.body = body.into();
    parts
}

fn runtime_gemini_normalized_error_body(status: u16, body: &[u8]) -> Option<Vec<u8>> {
    if status < 400 {
        return None;
    }
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        let error = value.get("error").unwrap_or(&value);
        if error.is_object() {
            let message = runtime_gemini_error_message(error)
                .unwrap_or_else(|| format!("Gemini upstream returned HTTP {status}."));
            let (error_type, code) = runtime_gemini_openai_error_kind(status, &value, body);
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
    let (error_type, code) = runtime_gemini_openai_error_kind(status, &gemini_error, body);
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

fn runtime_gemini_error_message(value: &serde_json::Value) -> Option<String> {
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

fn runtime_gemini_openai_error_kind(
    status: u16,
    value: &serde_json::Value,
    body: &[u8],
) -> (&'static str, &'static str) {
    if runtime_gemini_body_has_terminal_quota(body)
        || runtime_gemini_value_has_terminal_quota(value)
        || runtime_gemini_plain_text_has_terminal_quota(body)
    {
        return ("insufficient_quota", "insufficient_quota");
    }
    if status == 429 || runtime_gemini_value_has_rate_limit(value) {
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

fn runtime_gemini_plain_text_has_terminal_quota(body: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(body) else {
        return false;
    };
    let lower = text.to_ascii_lowercase();
    lower.contains("quota")
        && (lower.contains("exhausted")
            || lower.contains("exceeded")
            || lower.contains("insufficient"))
}

fn runtime_gemini_google_quota_message(body: &[u8]) -> Option<String> {
    runtime_gemini_values_from_body(body)
        .iter()
        .find_map(runtime_gemini_google_quota_message_from_value)
}

fn runtime_gemini_google_quota_message_from_value(value: &serde_json::Value) -> Option<String> {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                let message = object
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| object.get("detail").and_then(serde_json::Value::as_str))
                    .or_else(|| object.get("error").and_then(serde_json::Value::as_str));
                let explicit_quota = ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(runtime_gemini_google_quota_code)
                });
                if explicit_quota {
                    return Some(
                        message
                            .unwrap_or("Gemini account quota was exhausted.")
                            .to_string(),
                    );
                }
                stack.extend(object.values());
            }
            serde_json::Value::Array(values) => {
                stack.extend(values);
            }
            _ => {}
        }
    }
    None
}

fn runtime_gemini_google_quota_code(code: &str) -> bool {
    matches!(
        code.trim().to_ascii_lowercase().as_str(),
        "resource_exhausted"
            | "quota_exhausted"
            | "quota_exceeded"
            | "rate_limit_exceeded"
            | "rate_limit_exceeded_error"
    )
}

fn runtime_gemini_values_from_body(body: &[u8]) -> Vec<serde_json::Value> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        return vec![value];
    }
    let Ok(text) = std::str::from_utf8(body) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("data:"))
        .map(str::trim)
        .filter(|line| !line.is_empty() && *line != "[DONE]")
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect()
}

fn runtime_gemini_retry_after_header_ms(value: &str) -> Option<u64> {
    value
        .trim()
        .parse::<u64>()
        .ok()
        .map(|seconds| seconds.saturating_mul(1_000))
        .filter(|delay_ms| *delay_ms > 0)
}

fn runtime_gemini_retry_delay_ms_from_body(body: &[u8]) -> Option<u64> {
    runtime_gemini_values_from_body(body)
        .iter()
        .filter_map(runtime_gemini_retry_delay_ms_from_value)
        .max()
}

fn runtime_gemini_retry_delay_ms_from_value(value: &serde_json::Value) -> Option<u64> {
    let mut best = None;
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(delay_ms) = object
                    .get("retryDelay")
                    .and_then(serde_json::Value::as_str)
                    .and_then(runtime_gemini_duration_ms)
                {
                    runtime_gemini_update_max_ms(&mut best, delay_ms);
                }
                for key in ["message", "detail", "error"] {
                    if let Some(delay_ms) = object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .and_then(runtime_gemini_retry_delay_ms_from_message)
                    {
                        runtime_gemini_update_max_ms(&mut best, delay_ms);
                    }
                }
                if ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(runtime_gemini_google_rate_limit_code)
                }) {
                    runtime_gemini_update_max_ms(
                        &mut best,
                        RUNTIME_GEMINI_CLOUD_CODE_DEFAULT_RETRY_MS,
                    );
                }
                if runtime_gemini_object_mentions_quota_limit(object, "PerMinute") {
                    runtime_gemini_update_max_ms(&mut best, RUNTIME_GEMINI_PER_MINUTE_RETRY_MS);
                }
                stack.extend(object.values());
            }
            serde_json::Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
    best
}

fn runtime_gemini_value_has_terminal_quota(value: &serde_json::Value) -> bool {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(runtime_gemini_google_terminal_quota_code)
                }) {
                    return true;
                }
                if runtime_gemini_object_mentions_quota_limit(object, "PerDay")
                    || runtime_gemini_object_mentions_quota_limit(object, "Daily")
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

fn runtime_gemini_value_has_rate_limit(value: &serde_json::Value) -> bool {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(runtime_gemini_google_rate_limit_code)
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

fn runtime_gemini_update_max_ms(best: &mut Option<u64>, candidate: u64) {
    if candidate == 0 {
        return;
    }
    *best = Some(best.map_or(candidate, |current| current.max(candidate)));
}

fn runtime_gemini_duration_ms(value: &str) -> Option<u64> {
    let value = value.trim();
    if let Some(number) = value.strip_suffix("ms") {
        return runtime_gemini_parse_positive_float(number).map(|millis| {
            let millis = millis.ceil();
            if millis > u64::MAX as f64 {
                u64::MAX
            } else {
                millis as u64
            }
        });
    }
    if let Some(number) = value.strip_suffix('s') {
        return runtime_gemini_parse_positive_float(number).map(|seconds| {
            let millis = (seconds * 1_000.0).ceil();
            if millis > u64::MAX as f64 {
                u64::MAX
            } else {
                millis as u64
            }
        });
    }
    None
}

fn runtime_gemini_parse_positive_float(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn runtime_gemini_retry_delay_ms_from_message(message: &str) -> Option<u64> {
    let lower = message.to_ascii_lowercase();
    ["please retry in ", "suggested retry after "]
        .into_iter()
        .find_map(|marker| {
            let start = lower.find(marker)? + marker.len();
            runtime_gemini_duration_token_ms(&lower[start..])
        })
}

fn runtime_gemini_duration_token_ms(value: &str) -> Option<u64> {
    let value = value.trim_start();
    let number_len = value
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .map(char::len_utf8)
        .sum::<usize>();
    if number_len == 0 {
        return None;
    }
    let number = &value[..number_len];
    let suffix = &value[number_len..];
    if suffix.starts_with("ms") {
        runtime_gemini_duration_ms(&format!("{number}ms"))
    } else if suffix.starts_with('s') {
        runtime_gemini_duration_ms(&format!("{number}s"))
    } else {
        None
    }
}

fn runtime_gemini_object_mentions_quota_limit(
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
            .is_some_and(|metadata| runtime_gemini_object_mentions_quota_limit(metadata, needle))
}

fn runtime_gemini_google_rate_limit_code(code: &str) -> bool {
    matches!(
        code.trim().to_ascii_lowercase().as_str(),
        "rate_limit_exceeded" | "rate_limit_exceeded_error"
    )
}

fn runtime_gemini_google_terminal_quota_code(code: &str) -> bool {
    matches!(
        code.trim().to_ascii_lowercase().as_str(),
        "quota_exhausted" | "insufficient_g1_credits_balance" | "insufficient_quota"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_google_resource_exhausted_is_quota_blocked() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": 429,
                "message": "Quota exceeded for quota metric.",
                "status": "RESOURCE_EXHAUSTED"
            }
        }))
        .unwrap();

        assert_eq!(
            runtime_gemini_google_quota_message(&body).as_deref(),
            Some("Quota exceeded for quota metric.")
        );
        let parts = RuntimeHeapTrimmedBufferedResponseParts {
            status: 429,
            headers: Vec::new(),
            body: body.into(),
        };
        assert!(runtime_gemini_buffered_parts_are_quota_blocked(429, &parts));
    }

    #[test]
    fn gemini_rate_limit_retry_delay_uses_defaults_and_retry_after_cap() {
        assert_eq!(runtime_gemini_retry_delay_ms(None, b"", 0), Some(5_000));
        assert_eq!(runtime_gemini_retry_delay_ms(None, b"", 1), Some(10_000));
        assert_eq!(
            runtime_gemini_retry_delay_ms(Some("3"), b"", 0),
            Some(5_000)
        );
        assert_eq!(
            runtime_gemini_retry_delay_ms(Some("99"), b"", 0),
            Some(99_000)
        );
        assert_eq!(
            runtime_gemini_retry_delay_ms(Some("999"), b"", 0),
            Some(300_000)
        );
        assert_eq!(
            runtime_gemini_retry_delay_ms(Some("bad"), b"", 8),
            Some(30_000)
        );
        assert_eq!(runtime_gemini_retry_delay_ms(None, b"", 9), None);
    }

    #[test]
    fn gemini_rate_limit_retry_delay_reads_google_retry_info_from_json() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": 429,
                "message": "Resource exhausted, please try again later.",
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.RetryInfo",
                    "retryDelay": "34.074824224s"
                }]
            }
        }))
        .unwrap();

        assert_eq!(runtime_gemini_retry_delay_ms(None, &body, 0), Some(34_075));
    }

    #[test]
    fn gemini_rate_limit_retry_delay_reads_google_retry_info_from_sse() {
        let body = concat!(
            "data: {\"error\":{\"code\":429,\"message\":\"Please retry in 12.5s.\",",
            "\"status\":\"RESOURCE_EXHAUSTED\"}}\n\n"
        );

        assert_eq!(
            runtime_gemini_retry_delay_ms(None, body.as_bytes(), 0),
            Some(12_500)
        );
    }

    #[test]
    fn gemini_rate_limit_retry_delay_defaults_cloud_code_rate_limit_to_ten_seconds() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": 429,
                "message": "Rate limit exceeded.",
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                    "domain": "cloudcode-pa.googleapis.com",
                    "reason": "RATE_LIMIT_EXCEEDED"
                }]
            }
        }))
        .unwrap();

        assert_eq!(runtime_gemini_retry_delay_ms(None, &body, 0), Some(10_000));
    }

    #[test]
    fn gemini_terminal_quota_body_disables_rate_limit_retry() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": 429,
                "message": "Quota exhausted.",
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                    "domain": "cloudcode-pa.googleapis.com",
                    "reason": "QUOTA_EXHAUSTED"
                }]
            }
        }))
        .unwrap();

        assert!(runtime_gemini_body_has_terminal_quota(&body));
    }

    #[test]
    fn gemini_structured_terminal_quota_error_normalizes_to_openai_shape() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": 429,
                "message": "Quota exhausted.",
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                    "reason": "QUOTA_EXHAUSTED"
                }]
            }
        }))
        .unwrap();
        let parts = RuntimeHeapTrimmedBufferedResponseParts {
            status: 429,
            headers: vec![("content-type".to_string(), b"application/json".to_vec())],
            body: body.into(),
        };

        let normalized = runtime_gemini_normalized_error_parts(429, parts);
        let value: serde_json::Value = serde_json::from_slice(&normalized.body).unwrap();

        assert_eq!(value["error"]["type"], "insufficient_quota");
        assert_eq!(value["error"]["code"], "insufficient_quota");
        assert_eq!(value["error"]["message"], "Quota exhausted.");
        assert!(value["error"]["gemini_error"].is_object());
    }

    #[test]
    fn gemini_structured_rate_limit_error_normalizes_to_openai_shape() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": 429,
                "message": "Rate limit exceeded.",
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                    "reason": "RATE_LIMIT_EXCEEDED"
                }]
            }
        }))
        .unwrap();
        let parts = RuntimeHeapTrimmedBufferedResponseParts {
            status: 429,
            headers: vec![("content-type".to_string(), b"application/json".to_vec())],
            body: body.into(),
        };

        let normalized = runtime_gemini_normalized_error_parts(429, parts);
        let value: serde_json::Value = serde_json::from_slice(&normalized.body).unwrap();

        assert_eq!(value["error"]["type"], "rate_limit_error");
        assert_eq!(value["error"]["code"], "rate_limit_exceeded");
    }

    #[test]
    fn gemini_plain_text_rate_limit_error_normalizes_to_openai_shape() {
        let original = b"try later".to_vec();
        let parts = RuntimeHeapTrimmedBufferedResponseParts {
            status: 429,
            headers: vec![("content-type".to_string(), b"text/plain".to_vec())],
            body: original.clone().into(),
        };

        let normalized = runtime_gemini_normalized_error_parts(429, parts);
        let value: serde_json::Value = serde_json::from_slice(&normalized.body).unwrap();

        assert_eq!(value["error"]["type"], "rate_limit_error");
        assert_eq!(value["error"]["code"], "rate_limit_exceeded");
        assert_eq!(value["error"]["message"], "try later");
        assert_eq!(value["error"]["gemini_error"]["status"], 429);
    }

    #[test]
    fn gemini_plain_text_quota_error_normalizes_as_insufficient_quota() {
        let parts = RuntimeHeapTrimmedBufferedResponseParts {
            status: 429,
            headers: vec![("content-type".to_string(), b"text/plain".to_vec())],
            body: b"Quota exhausted for this account".to_vec().into(),
        };

        let normalized = runtime_gemini_normalized_error_parts(429, parts);
        let value: serde_json::Value = serde_json::from_slice(&normalized.body).unwrap();

        assert_eq!(value["error"]["type"], "insufficient_quota");
        assert_eq!(value["error"]["code"], "insufficient_quota");
    }
}
