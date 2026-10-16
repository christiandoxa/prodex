# Provider quota percentage math and Codex header text in Mojo

The provider quota-to-Codex HTTP adapter now delegates quota utilization
percentages and one-decimal wire formatting to the compiled provider
constraints kernel. Gemini remaining-fraction percentages and
OpenAI-compatible limit/remaining fractions use separate tagged operations
with an explicit, versioned ABI.

Mojo owns the applicability of an OpenAI-compatible positive finite limit,
missing inputs, the used-percentage calculations, clamping to 0..100%,
positive half-away-from-zero rounding to tenths, decimal formatting without
unnecessary trailing `.0`, and historical Gemini NaN/infinity handling.
The previously separate Rust `gemini_bucket_used_percent`, `format_percent`,
valid-limit gate, percentage formula, clamp, and decimal formatting were
deleted without a Rust fallback.

Rust retains HTTP header and JSON scalar acquisition, strict f64 decoding,
UTC/RFC3339 timestamp decoding, and typed header assembly. The real runtime
caller has an explicit rounded-percent and saturation regression. Direct
compiled-Mojo tests exercise missing fields, bounds, fractions, finite/nonfinite
inputs, decimal precision, ABI versions/modes, malformed presence tags and
insufficient output capacity. The no-fallback guard prevents restoring the
Rust formulas or formatters.

The project-wide 75% Mojo production-share target remains outstanding.
