# Shared provider catalog integer validation in Mojo (2026-10-22)

## Production ownership

The native DeepSeek, Gemini and local-provider catalog paths now use the same
Mojo integer parser as external provider configuration. Their shared Rust
catalog parser, Unicode-whitespace check and lower-bound decision are deleted.
The existing Mojo parser also returns the original integer failure category;
its existing pair-normalizer consumers keep the same status/value behavior.

`prodex_provider_catalog_uint_v1` is a narrow scalar export that avoids applying
the pair normalizer's compact/context clamping to an individual value. It
validates version, borrowed UTF-8 and output shape before writing. Rust checks
the returned tags and preserves provider/key error context and the original
integer source-error text. Parsing and error classification have one Mojo
authority, with no Rust recomputation or fallback.

Behavior includes full-width unsigned values, leading plus/zeros, Unicode
whitespace rejection before parsing, a strict lower bound greater than one,
and first integer-error order. For example an overflowing prefix followed by
`x` still reports overflow; an earlier `x` reports an invalid digit. Long
leading-zero inputs keep the old accepted range without a new text-size cap.
Serde/TOML acquisition and `anyhow` error construction remain Rust boundaries.

## Verification

Direct Mojo assertions cover successful/boundary values, all error classes and
atomic ABI rejection. Compiled tests compare 84 prefix/suffix combinations
against Rust's test-only integer parser, preserve the source-error text, check
a value with over one million leading zeros and reject malformed ABI views.
Caller tests assert exact formatted errors for all three provider labels and
both catalog fields. Existing provider configuration tests exercise the real
catalog preparation flows.

Compiled Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. The changed-test runner uses its default features.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/provider_catalog_uint_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib super_provider_config::`
- `cargo test --locked -q -p prodex-app --lib runtime_catalog_config::tests -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_deepseek_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_gemini_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_local_provider_config:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/compat/check-upstream-baseline.mjs`
- `node scripts/compat/capture-replay-fixture-tests.mjs`
- `npm run docs`
- `npm run test:changed`
- `npm run ci -- --jobs 1`
- `git diff --check`

Native non-Linux and live-provider execution are outside this offline unit.
The additional checked Rust ABI shim slightly reduces the raw source share
from 36.304688936071585% to 36.303050159420174% (136,290 Mojo / 239,133 Rust LOC).
The production Rust parser is nevertheless removed; counting rules and guards
are unchanged. The 75% project target remains unmet.

Direct Mojo assertions, all five configuration bridge tests, both shared
caller tests, four DeepSeek, six Gemini and five local-provider tests pass.
Strict workspace Clippy, no-fallback self-test, authority, ownership,
source-share, size, crate-boundary, secret-boundary and offline compatibility
checks pass.

The initial full CI attempt failed three upstream mock tests before obtaining
an HTTP response. Their shared request reader treated a 10 ms socket read
timeout as complete headers and could reply before receiving the request.
A deterministic injected-timeout regression reproduced the problem: the old
reader consumed zero of 49 header bytes. The fixed helper retries transient
reads until complete headers under a two-second deadline and rejects early
EOF. Both TCP fixtures use a short read timeout so that deadline remains
effective. The helper and regression live together in the extracted test file;
the main file's size cap is tightened from 1980 to 1972 lines.

The regression failed before the fix, then all 21 focused upstream tests
passed. The full local gate is rerun with that test-fixture correction.

The corrected full local CI gate passes, including all 685 runtime-launch tests
and 43 auto-rotate integration tests. The default changed-test runner passes.
CI passes 282 Node tests and skips its native Windows installer test on Linux.
The original failed gate and failing pre-fix regression remain in the retained
validation evidence; no failure is counted as passing coverage.
