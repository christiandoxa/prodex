pub(super) const SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES: usize = 1024;
pub(super) const SMART_CONTEXT_ADMISSION_MIN_BODY_BYTES: usize = 512;
pub(super) const SMART_CONTEXT_SHADOW_SAMPLE_BASIS_POINTS: u16 = 100;
pub(super) const SMART_CONTEXT_FALLBACK_CONTEXT_WINDOW_TOKENS: u64 = 32_000;
pub(super) const SMART_CONTEXT_RESERVED_OUTPUT_TOKENS: u64 = 4_096;
pub(super) const SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT: usize = 8;
pub(super) const SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT: usize = 16;
pub(super) const SMART_CONTEXT_TOKEN_CALIBRATION_PERSISTENCE_VERSION: u32 = 1;
pub(super) const SMART_CONTEXT_TOKEN_CALIBRATION_SAVE_DELAY_MS: u64 = 250;
pub(super) const SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT: usize = 16;
pub(super) const SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT: usize = 4;
pub(super) const SMART_CONTEXT_REWRITE_SAFETY_TTL_SECS: u64 = 6 * 60 * 60;
pub(super) const SMART_CONTEXT_HTTP_REWRITE_MAX_BYTES: usize = 256 * 1024;
pub(super) const SMART_CONTEXT_WEBSOCKET_REWRITE_MAX_BYTES: usize = 96 * 1024;
#[cfg(not(debug_assertions))]
pub(super) const SMART_CONTEXT_REWRITE_DEADLINE_MS: u64 = 100;
#[cfg(debug_assertions)]
pub(super) const SMART_CONTEXT_REWRITE_DEADLINE_MS: u64 = 5_000;
pub(super) const SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX: &str = "psc:";
pub(super) const SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX: &str = "psc static ";
pub(super) const SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY: &str =
    "prodex static context unchanged ";
pub(super) const RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS: [&str; 3] =
    ["instructions", "system", "developer"];
