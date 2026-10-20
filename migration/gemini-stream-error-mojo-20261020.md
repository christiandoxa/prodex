# Gemini embedded stream-error projection compiled in Mojo (2026-10-20)

## Production authority and compatibility

The `gemini_value_policy.mojo` production owner now also selects and
formats embedded Gemini stream error codes and messages through its
versioned JSON tree ABI. The Rust adapter acquires only the `error`
member using the existing Serde host bridge, so a successful SSE
payload is not serialized just to check error presence. Mojo decides
whether that field is present and produces either JSON `null` or a
validated `[code, message]` pair; the host only decodes the typed result.

The compiled policy preserves `status` before `code` based on **field
presence**, including null or invalid status values. Strings are
accepted verbatim, signed 64-bit integer values are rendered as decimal,
and floating point, exponent, out-of-range unsigned, boolean, object,
and array codes select the original provider default. A missing or
nonstring message selects the original diagnostic text, while an empty
string remains a valid message. The prior Rust status/code/message
selection and default rendering have been deleted; no Rust semantic
fallback is used.

## Verification

- `mojo build --emit object -I mojo/prodex_core -o /tmp/prodex-gemini-stream-error-20261020.o mojo/prodex_core/gemini_value_policy.mojo`: success.
- `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-provider-core --features mojo --lib -- --test-threads=1`: 513 tests passed.
- `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::gemini_value_policy::tests -- --test-threads=1`: 6 tests passed.
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`: passed.
- `node scripts/ci/mojo-production-share.mjs --check --json`: passed, with audited non-regression.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

New caller regressions cover absent and null error fields, Unicode,
empty strings, malformed/status-presence precedence, minimum and maximum
signed integers, out-of-range unsigned numbers, and decimal fractions.
The direct ABI test validates Serde canonical number spans and JSON
null emission for absent errors.

The official production share increases from 35.74326074615431% to
35.75250743965612% (129,753 Mojo and 233,167 Rust LOC). The 75% project
goal remains unmet. This stage does not change the denominator, counting
rules, required baseline or release policy.
