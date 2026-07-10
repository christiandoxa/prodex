# ADR 0992: Gemini OAuth pool debug redaction

## Status

Accepted.

## Context

Gemini OAuth pool state carries profile lists, response/tool/session affinity
bindings, quota headers, model cooldowns, model-unavailable markers, and model
preferences. Derived `Debug` output can leak profile names, response IDs,
session IDs, tool-call IDs, quota header values, model names, and timing data
through panic output, assertions, or diagnostics.

## Decision

`RuntimeGeminiOAuthPoolState` and `RuntimeGeminiModelPreference` use
hand-written `Debug` implementations that preserve pool and preference shape
while redacting profile, binding, quota, model, and timing details.

## Consequences

OAuth rotation, affinity, quota fallback, and model preference behavior are
unchanged. Diagnostics can still identify pool shape and collection sizes
without exposing account or request metadata. Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_tests.rs`.
