//! Gemini retry-delay parsing.

use prodex_mojo_core::provider_constraints::{
    gemini_invalid_stream_retry_delay_ms, gemini_retry_delay_ms, gemini_retry_inline_threshold_ms,
    gemini_should_inline_retry,
};
#[path = "retry_delay/duration.rs"]
mod duration;

use self::duration::{
    gemini_provider_core_duration_ms, gemini_provider_core_retry_after_header_ms,
    gemini_provider_core_retry_delay_ms_from_message,
};
use super::gemini_provider_core_values_from_body;
use crate::mojo_json::Document;
use prodex_mojo_core::json::{
    GeminiRetryCandidateKind, gemini_retry_candidates, gemini_retry_delay_max,
};

pub fn gemini_provider_core_retry_delay_ms_from_body(body: &[u8]) -> Option<u64> {
    let mut delays_ms = Vec::new();
    for value in gemini_provider_core_values_from_body(body) {
        let mut document = Document::default();
        document.push(&value, None, "");
        let raw =
            std::str::from_utf8(&document.raw).expect("Serde Gemini retry body must remain UTF-8");
        let plan = gemini_retry_candidates(&document.nodes, raw)
            .expect("compiled Mojo Gemini nested retry candidate scan failed");
        if plan.floor_ms > 0 {
            delays_ms.push(plan.floor_ms);
        }
        for field in plan.fields {
            let candidate = match field.kind {
                GeminiRetryCandidateKind::ExplicitDuration => {
                    gemini_provider_core_duration_ms(field.text)
                }
                GeminiRetryCandidateKind::MessageText => {
                    gemini_provider_core_retry_delay_ms_from_message(field.text)
                }
            };
            if let Some(candidate) = candidate {
                delays_ms.push(candidate);
            }
        }
    }
    gemini_retry_delay_max(&delays_ms)
        .expect("compiled Mojo Gemini retry-delay maximum policy returned invalid output")
}

pub fn gemini_provider_core_retry_delay_ms(
    retry_after: Option<&str>,
    body: &[u8],
    retry_index: usize,
) -> Option<u64> {
    gemini_retry_delay_ms(
        retry_after.and_then(gemini_provider_core_retry_after_header_ms),
        gemini_provider_core_retry_delay_ms_from_body(body),
        retry_index,
    )
    .expect("Mojo Gemini retry-delay policy returned invalid output")
}

pub fn gemini_provider_core_should_inline_rate_limit_retry(delay_ms: u64) -> bool {
    gemini_should_inline_retry(delay_ms)
        .expect("Mojo Gemini inline-retry policy returned invalid output")
}

pub fn gemini_provider_core_invalid_stream_retry_delay_ms(retry_index: usize) -> u64 {
    gemini_invalid_stream_retry_delay_ms(retry_index)
        .expect("Mojo Gemini invalid-stream retry policy returned invalid output")
}

pub fn gemini_provider_core_inline_retry_threshold_ms() -> u64 {
    gemini_retry_inline_threshold_ms()
        .expect("Mojo Gemini inline-retry threshold policy returned invalid output")
}
