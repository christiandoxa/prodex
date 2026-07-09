use super::local_rewrite_gemini_quota::runtime_gemini_response_retryable_quota;

pub(super) const RUNTIME_GEMINI_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS: u64 = 30_000;
const RUNTIME_GEMINI_INVALID_STREAM_RETRY_BASE_DELAY_MS: u64 = 1_000;

pub(super) fn runtime_gemini_should_inline_rate_limit_retry(delay_ms: u64) -> bool {
    delay_ms > 0 && delay_ms <= RUNTIME_GEMINI_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS
}

pub(super) fn runtime_gemini_invalid_stream_retry_delay_ms(retry_index: usize) -> u64 {
    RUNTIME_GEMINI_INVALID_STREAM_RETRY_BASE_DELAY_MS.saturating_mul(1_u64 << retry_index.min(8))
}

pub(super) fn runtime_gemini_should_rotate_after_quota_response(
    status: u16,
    quota_blocked: bool,
    hard_affinity: bool,
    quota_fallback_allowed: bool,
    attempt_index: usize,
    attempt_count: usize,
) -> bool {
    quota_blocked
        && runtime_gemini_response_retryable_quota(status)
        && (!hard_affinity || quota_fallback_allowed)
        && attempt_index + 1 < attempt_count
}
