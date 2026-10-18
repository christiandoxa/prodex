//! Gemini quota, retry, and rate-limit classifiers.

mod body;
mod retry_delay;

pub use self::retry_delay::{
    gemini_provider_core_inline_retry_threshold_ms,
    gemini_provider_core_invalid_stream_retry_delay_ms, gemini_provider_core_retry_delay_ms,
    gemini_provider_core_retry_delay_ms_from_body,
    gemini_provider_core_should_inline_rate_limit_retry,
};

use self::body::gemini_provider_core_values_from_body;
use crate::mojo_json::Document;

pub fn gemini_provider_core_response_retryable_quota(status: u16) -> bool {
    prodex_mojo_core::json::gemini_quota_retryable_http_status(status)
        .expect("compiled Mojo Gemini quota retryable status returned invalid output")
}

pub fn gemini_provider_core_body_has_terminal_quota(body: &[u8]) -> bool {
    gemini_provider_core_values_from_body(body)
        .iter()
        .any(|value| gemini_provider_core_quota_body_plan(value).terminal_quota)
}

pub fn gemini_provider_core_google_quota_message(body: &[u8]) -> Option<String> {
    gemini_provider_core_values_from_body(body)
        .iter()
        .find_map(|value| gemini_provider_core_quota_body_plan(value).quota_message)
}

pub fn gemini_provider_core_should_rotate_after_quota_response(
    status: u16,
    quota_blocked: bool,
    hard_affinity: bool,
    _quota_fallback_allowed: bool,
    attempt_index: usize,
    attempt_count: usize,
) -> bool {
    prodex_mojo_core::json::gemini_quota_should_rotate(
        status,
        quota_blocked,
        hard_affinity,
        attempt_index,
        attempt_count,
    )
    .expect("compiled Mojo Gemini quota rotation decision returned invalid output")
}

pub(super) fn gemini_provider_core_quota_body_plan(
    value: &serde_json::Value,
) -> prodex_mojo_core::json::GeminiQuotaBodyPlan {
    let mut document = Document::default();
    document.push(value, None, "");
    let raw =
        std::str::from_utf8(&document.raw).expect("Serde Gemini error body must remain UTF-8");
    prodex_mojo_core::json::gemini_quota_body_plan(&document.nodes, raw)
        .expect("compiled Mojo Gemini quota classifier returned invalid output")
}

pub(super) fn gemini_provider_core_value_has_terminal_quota(value: &serde_json::Value) -> bool {
    gemini_provider_core_quota_body_plan(value).terminal_quota
}

pub(super) fn gemini_provider_core_value_has_rate_limit(value: &serde_json::Value) -> bool {
    gemini_provider_core_quota_body_plan(value).rate_limit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_quota_rotation_preserves_affinity_status_and_attempt_budget() {
        for status in [200, 400, 401, 403, 404, 429, 500] {
            let eligible = matches!(status, 403 | 429);
            assert_eq!(
                gemini_provider_core_response_retryable_quota(status),
                eligible
            );
            for fallback in [false, true] {
                assert_eq!(
                    gemini_provider_core_should_rotate_after_quota_response(
                        status, true, false, fallback, 0, 2
                    ),
                    eligible,
                );
                for (blocked, hard, index, count) in [
                    (false, false, 0, 2),
                    (true, true, 0, 2),
                    (true, false, 1, 2),
                    (true, false, 0, 0),
                    (true, false, usize::MAX, usize::MAX),
                ] {
                    assert!(!gemini_provider_core_should_rotate_after_quota_response(
                        status, blocked, hard, fallback, index, count,
                    ));
                }
            }
        }
    }

    #[test]
    fn compiled_gemini_quota_tree_preserves_nested_precedence_daily_limits_and_sse_frames() {
        for (value, terminal, rate, message) in [
            (
                serde_json::json!({"error":{"code":429,"message":"Quota exhausted.",
                    "details":[{"reason":"QUOTA_EXHAUSTED"}]}}),
                true,
                false,
                Some("Gemini account quota was exhausted."),
            ),
            (
                serde_json::json!({"error":{"status":" RESOURCE_EXHAUSTED ","message":"limit 🧭"}}),
                true,
                false,
                Some("limit 🧭"),
            ),
            (
                serde_json::json!({"error":{"code":" Rate_Limit_Exceeded_Error ","detail":"slow"}}),
                false,
                true,
                Some("slow"),
            ),
            (
                serde_json::json!({"a":{"status":"quota_exceeded","message":"left"},
                "z":{"code":"resource_exhausted","message":"right"}}),
                true,
                false,
                Some("right"),
            ),
            (
                serde_json::json!({"a":[{"status":"quota_exceeded","message":"first"},
                {"status":"quota_exhausted","message":"second"}]}),
                true,
                false,
                Some("second"),
            ),
            (
                serde_json::json!({"error":{"status":"quota_exhausted","message":""}}),
                true,
                false,
                Some(""),
            ),
            (
                serde_json::json!({"error":{"status":"quota_exhausted","message":1,"detail":"fallback"}}),
                true,
                false,
                Some("fallback"),
            ),
            (
                serde_json::json!({"error":{"status":"quota_exhausted","message":null,"detail":null}}),
                true,
                false,
                Some("Gemini account quota was exhausted."),
            ),
            (
                serde_json::json!({"metadata":{"quotaId":"GenerateRequestsPerDayPerProject"}}),
                true,
                false,
                None,
            ),
            (
                serde_json::json!({"error":{"details":[{"metadata":{"quota_limit":"QueriesDaily"}}]}}),
                true,
                false,
                None,
            ),
            (
                serde_json::json!({"metadata":{"quotaId":"GenerateRequestsPerMinutePerProject"}}),
                false,
                false,
                None,
            ),
            (
                serde_json::json!({"error":{"status":"INSUFFICIENT_QUOTA"}}),
                true,
                false,
                None,
            ),
            (
                serde_json::json!({"error":{"status":"OK","message":"not limited"}}),
                false,
                false,
                None,
            ),
            (
                serde_json::json!({"error":{"status":429,"message":"not a code"}}),
                false,
                false,
                None,
            ),
        ] {
            let plan = gemini_provider_core_quota_body_plan(&value);
            assert_eq!(plan.terminal_quota, terminal, "{value:?}");
            assert_eq!(plan.rate_limit, rate, "{value:?}");
            assert_eq!(plan.quota_message.as_deref(), message, "{value:?}");
        }

        let frames = b"data: {\"error\":{\"code\":\"rate_limit_exceeded\",\"message\":\"wait\"}}\n\
data: not-json\n\
data: {\"error\":{\"code\":\"resource_exhausted\",\"message\":\"daily\"}}\n\
data: [DONE]\n";
        assert!(gemini_provider_core_body_has_terminal_quota(frames));
        assert_eq!(
            gemini_provider_core_google_quota_message(frames).as_deref(),
            Some("wait")
        );
    }

    #[test]
    fn compiled_gemini_retry_candidates_keep_duration_grammar_rate_minimum_and_sse_maximum() {
        for (json, expected) in [
            (serde_json::json!({"retryDelay":"1.5s"}), Some(1500)),
            (
                serde_json::json!({"error":{"retryDelay":"9e1s"}}),
                Some(90_000),
            ),
            (
                serde_json::json!({"error":{"retryDelay":"250ms"}}),
                Some(250),
            ),
            (
                serde_json::json!({"message":"PLEASE RETRY IN 2.75S"}),
                Some(2750),
            ),
            (
                serde_json::json!({"detail":"SUGGESTED RETRY AFTER 250MS"}),
                Some(250),
            ),
            (
                serde_json::json!({"error":{"reason":"RATE_LIMIT_EXCEEDED_ERROR"}}),
                Some(10_000),
            ),
            (
                serde_json::json!({"metadata":{"quotaId":"RequestsPerMinute"}}),
                Some(60_000),
            ),
            (
                serde_json::json!({"metadata":{"quota_limit":"RequestsPerMinute"}}),
                Some(60_000),
            ),
            (
                serde_json::json!({"error":{"retryDelay":"70s","code":"RATE_LIMIT_EXCEEDED"}}),
                Some(70_000),
            ),
            (
                serde_json::json!({"metadata":{"quotaId":"RequestsPerMinute"},"retryDelay":"61s"}),
                Some(61_000),
            ),
            (
                serde_json::json!({"metadata":{"quotaId":"RequestsPerDay"}}),
                None,
            ),
            (
                serde_json::json!({"retryDelay":"0ms","message":"retry later"}),
                None,
            ),
            (serde_json::json!({"retryDelay":null,"status":429}), None),
            (
                serde_json::json!({"unrelated":{"retryDelay":"0.01s"}}),
                Some(10),
            ),
        ] {
            let body = serde_json::to_vec(&json).unwrap();
            assert_eq!(
                gemini_provider_core_retry_delay_ms_from_body(&body),
                expected,
                "{json:?}",
            );
        }
        let frames = b"data: {\"error\":{\"retryDelay\":\"1.5s\"}}\n\
data: not-json\n\
data: {\"error\":{\"message\":\"SUGGESTED RETRY AFTER 90S\"}}\n\
data: {\"metadata\":{\"quotaId\":\"QueriesPerMinute\"}}\n\
data: [DONE]\n";
        assert_eq!(
            gemini_provider_core_retry_delay_ms_from_body(frames),
            Some(90_000)
        );
        let frames = b"data: {\"error\":{\"retryDelay\":\"1.5s\"}}\n\
data: {\"metadata\":{\"quotaId\":\"QueriesPerMinute\"}}\n";
        assert_eq!(
            gemini_provider_core_retry_delay_ms_from_body(frames),
            Some(60_000)
        );
    }

    #[test]
    fn gemini_quota_code_casefold_preserves_trimmed_contract() {
        let terminal = br#"{"error":{"status":" RESOURCE_EXHAUSTED "}}"#;
        assert!(gemini_provider_core_body_has_terminal_quota(terminal));
        assert_eq!(
            gemini_provider_core_google_quota_message(
                br#"{"error":{"status":" QUOTA_EXCEEDED ","message":"limit reached"}}"#
            )
            .as_deref(),
            Some("limit reached")
        );
        assert!(gemini_provider_core_value_has_rate_limit(
            &serde_json::json!({
                "error": {"code": " RATE_LIMIT_EXCEEDED_ERROR "}
            })
        ));
        assert!(!gemini_provider_core_value_has_rate_limit(
            &serde_json::json!({
                "error": {"code": "rate limit exceeded"}
            })
        ));
    }
}
