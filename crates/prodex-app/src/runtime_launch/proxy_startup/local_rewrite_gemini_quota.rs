use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use prodex_provider_core::{
    gemini_provider_core_body_has_terminal_quota, gemini_provider_core_google_quota_message,
    gemini_provider_core_normalized_error_body, gemini_provider_core_response_retryable_quota,
};
use runtime_proxy_crate::extract_runtime_proxy_quota_message;

pub(super) fn runtime_gemini_buffered_parts_are_quota_blocked(
    status: u16,
    parts: &RuntimeHeapTrimmedBufferedResponseParts,
) -> bool {
    gemini_provider_core_response_retryable_quota(status)
        && (extract_runtime_proxy_quota_message(&parts.body).is_some()
            || gemini_provider_core_google_quota_message(&parts.body).is_some())
        || runtime_provider_error_class(RuntimeProviderBridgeKind::Gemini, status, &parts.body)
            == RuntimeProviderErrorClass::Quota
}

pub(super) fn runtime_gemini_normalized_error_parts(
    status: u16,
    mut parts: RuntimeHeapTrimmedBufferedResponseParts,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let Some(body) = gemini_provider_core_normalized_error_body(status, &parts.body) else {
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
            gemini_provider_core_google_quota_message(&body).as_deref(),
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
        assert_eq!(
            prodex_provider_core::gemini_provider_core_retry_delay_ms(None, b"", 0),
            Some(5_000)
        );
        assert_eq!(
            prodex_provider_core::gemini_provider_core_retry_delay_ms(None, b"", 1),
            Some(10_000)
        );
        assert_eq!(
            prodex_provider_core::gemini_provider_core_retry_delay_ms(Some("3"), b"", 0),
            Some(5_000)
        );
        assert_eq!(
            prodex_provider_core::gemini_provider_core_retry_delay_ms(Some("99"), b"", 0),
            Some(99_000)
        );
        assert_eq!(
            prodex_provider_core::gemini_provider_core_retry_delay_ms(Some("999"), b"", 0),
            Some(300_000)
        );
        assert_eq!(
            prodex_provider_core::gemini_provider_core_retry_delay_ms(Some("bad"), b"", 8),
            Some(30_000)
        );
        assert_eq!(
            prodex_provider_core::gemini_provider_core_retry_delay_ms(None, b"", 9),
            None
        );
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

        assert_eq!(
            prodex_provider_core::gemini_provider_core_retry_delay_ms(None, &body, 0),
            Some(34_075)
        );
    }

    #[test]
    fn gemini_rate_limit_retry_delay_reads_google_retry_info_from_sse() {
        let body = concat!(
            "data: {\"error\":{\"code\":429,\"message\":\"Please retry in 12.5s.\",",
            "\"status\":\"RESOURCE_EXHAUSTED\"}}\n\n"
        );

        assert_eq!(
            prodex_provider_core::gemini_provider_core_retry_delay_ms(None, body.as_bytes(), 0),
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

        assert_eq!(
            prodex_provider_core::gemini_provider_core_retry_delay_ms(None, &body, 0),
            Some(10_000)
        );
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

        assert!(gemini_provider_core_body_has_terminal_quota(&body));
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
