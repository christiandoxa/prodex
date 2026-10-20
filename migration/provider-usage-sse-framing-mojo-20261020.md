# Provider Usage SSE Framing Migrated to Mojo (2026-10-20)

## Production ownership

The compiled provider_usage.mojo owner now detects SSE data fields,
normalizes CRLF line endings, applies the one-ASCII-space-after-data
rule, trims trailing carriage returns and emits ordered borrowed
data-line spans plus frame-end markers through the versioned
prodex_provider_usage_sse_scan_v1 ABI. The measured first call and
exact-size second call share one compiled scanner implementation.
A preflight count rejects insufficient output capacity before a
partial record can be written.

Rust retains UTF-8 validation at the Serde boundary, original borrowed
payload ownership, construction of the frame's JSON text from
Mojo-selected slices, Serde JSON parsing, and application of the
previously compiled JSON usage extraction and latest-present usage
merge plans. The old Rust SSE line loop, prefix/space/CRLF extraction,
blank-frame detection and DONE-text special case were removed without
a Rust framing fallback. DONE remains ignored because it is not valid
JSON for the existing Serde parser, preserving behavior.

## Verification

- All 529 prodex-provider-core strict tests pass, including additional
  production caller regressions for multiline JSON, CRLF, comments,
  final data without a blank terminator, consecutive blank events,
  UTF-8/Unicode, DONE and malformed/irrelevant frames.
- Two direct core ABI tests verify exact borrowed spans and frame
  ordering, empty frames, end-of-input flush, version/pointer/flag
  validation, invalid UTF-8, and no output writes on rejected capacity.
- Direct Mojo ABI test verifies measure/write parity, exact record
  bounds, malformed flags and unchanged results on rejection.
- Source anti-fallback guard/self-test, Cargo formatting, diff check
  and official reachable-Mojo non-regression validation pass.

The canonical broad share rises from 35.836281199005576% to
35.851405897242934% (131,010 Mojo / 234,415 Rust production LOC).
The 75% project goal is still unmet. No counting rules, release
floor or baseline were changed, and remote CI green is not a
checkpoint prerequisite.
