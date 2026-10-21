# Gemini quota counter text and numeric records in Mojo (2026-10-22)

## Production ownership

The Gemini quota renderer now sends borrowed counter text and fraction facts
through `prodex_quota_gemini_bucket_records_v1`. Mojo trims Unicode whitespace,
parses checked signed integers, distinguishes absent/invalid counters and
computes the existing numeric plan. Rust's `trim().parse::<i64>()` helper is
deleted without a fallback. Output labels, percent/status rendering and
1,024-row chunking retain their established owners and behavior.

Both the parsed-counter Rust API and the new text API use one checked record
adapter, replacing eleven separate ABI field arrays with input/output records.
The existing Mojo numeric algorithm is extracted as a pure function; its old C
export remains a compatibility facade using that same function. The rounding
implementation likewise has one pure owner and an unchanged C facade. This
avoids duplicate exported symbols when the record unit imports the pure plan.

The versioned record ABI accepts at most 1,024 rows. It validates all source
tags, presence flags, UTF-8 text, alignment and output capacity before writing
any result. Rust verifies output presence/exhaustion tags and keeps borrowed
strings alive for the call. Numeric presence, non-finite fractions, signed
limits, invalid/absent amount fallback and saturation retain existing policy.
There is no allocation or host I/O in the Mojo kernel.

## Verification

Direct Mojo assertions cover Unicode trim, signed limits, invalid amounts,
empty batches and atomic ABI rejection. Bridge tests compare text inputs to
the parsed-counter API across absent/present, zero, negative and non-finite
fractions; whole-output assertions independently fix remaining, total,
percent and exhaustion fields. Full-size batch and malformed trailing-row
tests verify capacity and failure atomicity. Production renderer tests compare
numeric results and rendered status, main quota and bucket summaries. The
existing 1,025-bucket test preserves rendering after the ABI batch boundary.
Direct assertions also compare the unchanged C numeric facade's seven output
fields to the new record plan and exercise its rounding facade.

Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`; changed-test selection keeps default features.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/quota_gemini_amount_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-quota --lib quota::`
- `cargo test --locked -q -p prodex-quota`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `npm run ci -- --jobs 1`
- `git diff --check`

An intermediate linker check found duplicate rounding exports. Extracting the
pure rounding function fixed the linking failure without removing the old C
symbol or duplicating its algorithm. The failed evidence is retained.
Native non-Linux and live-provider execution remain outside this offline unit.
The 75% project goal remains active.

Direct Mojo assertions, all 21 quota bridge tests, all 109 quota tests and strict
workspace Clippy pass. No-fallback self-test, authority, ownership, source-share,
size and crate-boundary checks pass. Canonical source share is
36.3105684345082% (136,348 Mojo / 239,157 Rust production LOC).

The full local CI gate and changed-test runner pass. CI passes all 685
runtime-launch tests, 43 auto-rotate integration tests and 282 Node tests;
the native Windows installer test is skipped on Linux. The default changed
runner also runs all 109 quota tests. Feature-off zero-test core harnesses are
not counted as compiled ABI coverage.
