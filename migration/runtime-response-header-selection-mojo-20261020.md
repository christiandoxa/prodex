# Runtime HTTP Response Header Selection Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled response-forwarding owner now selects header values
through the versioned prodex_runtime_response_header_select_v1 ABI.
Three live operations share one borrowed header batch:

- The first header name matching case-insensitively, with a Unicode-trimmed
  value. A first matching name with an empty value blocks later duplicates.
- The first usable binary Content-Type, skipping invalid UTF-8, blank
  values and unrelated header names. This reuses the existing compiled
  Content-Type validation policy and retains header-name trimming.
- Whether any exact Content-Type header requires per-chunk SSE flushing.
  Header-name matching and SSE MIME classification are compiled Mojo policy.

Rust retains the original HTTP header buffers, system/transport access,
checked length conversion, typed ABI result validation and application
of the selected borrowed value slice or boolean. The previous Rust
find/find_map/any selection, redundant header-value Unicode trimming and
duplicated SSE search were deleted. No Rust policy fallback was added.

## Verification

- All 421 strict prodex-runtime-proxy tests pass, including three new
  caller regressions for Unicode trimming, empty first-match barriers,
  malformed binary values, case folding, exact header names and SSE.
- Two direct core ABI tests cover typed selection, malformed late
  header rows, incorrect version, invalid operation and output buffer
  integrity on rejection.
- Direct Mojo ABI test confirms all three operations and failure modes.
- Mojo no-fallback guard/self-test, Cargo formatting, diff and audited
  production-share non-regression checks pass.

The official production share rises from 35.83402093790177% to
35.83891367008976% (130,802 Mojo / 234,170 Rust production LOC).
The 75% project target remains unmet. Counting definitions and release
rules were not modified. No remote CI green prerequisite for checkpoint.
