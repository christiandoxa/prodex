//! Gemini compact request/summary helpers.

mod local;
mod semantic;

pub use self::local::{
    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_SUMMARY_PREFIX, gemini_provider_core_compact_response_body,
    gemini_provider_core_local_compact_summary,
    gemini_provider_core_semantic_compact_continuation_summary,
};
pub use self::semantic::{
    gemini_provider_core_semantic_compact_instructions,
    gemini_provider_core_semantic_compact_request_body,
    gemini_provider_core_semantic_compact_summary,
};
