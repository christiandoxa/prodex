# CLI presentation compiled in Mojo (2026-10-22)

Nine after-help bodies, 21 canonical process labels and the five-command runtime
classification now belong to a compiled CLI presentation owner. Rust's help
constants, label literals and runtime match are removed without a fallback.
Clap parsing, exhaustive variant/tag marshalling, host allocation and static
label storage remain in Rust. The public doctor-tail byte constant, labels,
runtime flags, aliases, defaults and static label lifetimes retain their contracts.

The version-1 text ABI uses the existing measured-text allocator and validates
versions, operations, tags, phase flags, pointers, alignment and capacity before
publishing output. A versioned scalar result owns runtime classification. Rust
keeps returned label strings in the existing OnceLock pattern; it does not borrow
foreign heap objects or recompute text or classification. Help bodies move into
Clap-owned strings. The shared text adapter is enabled for the runtime feature,
and both runtime-only and full-core bridge configurations are tested. No new
dependency or counting rule is introduced.

The original Rust compiler captured all 9,775 help bytes. Exact literal comparison,
frozen caller byte hashes for every help body, all 21 canonical parsed commands
and direct Mojo checks of all 30 text payloads preserve the presentation contract.
A CRLF compiler probe found that multiline Mojo literals retain carriage returns.
Adjacent static literals now encode LF explicitly, and both LF and CRLF probes
pass without adding a runtime normalizer. These probes test source line endings;
they do not substitute for native execution on another operating system.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default CI/changed tests omit both and any archive override. Commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/cli_presentation_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib cli_presentation::tests::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib cli_presentation::tests::`
- `cargo test --locked -q -p prodex-cli presentation:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-cli -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib process_classification_uses_canonical_cli_semantics -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib reports::info:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

Two caller regressions pass on the original Rust implementation. The final
LF-escaped source passes direct Mojo, two bridge tests in each feature mode,
all 155 CLI tests, the application process-classification caller and 27 info
tests. Strict workspace Clippy and formatting pass. Full default CI passes
1,443 Rust tests in 537.00 seconds. Default changed tests pass 155 Rust tests
in 76.30 seconds; their no-feature core invocation runs zero tests, so the
two explicit core/runtime bridge configurations above supply that coverage.
Source/ownership guards, documentation, staged churn and whitespace checks pass.
Evidence retains a corrected unit-test import, the canonical `use --profile`
fixture correction, the failed multiline CRLF probe and its passing replacement.

Canonical share is 36.54009398411737% (138,176 Mojo / 239,973 Rust production LOC),
removing 73 Rust production LOC while transferring text and classification.
The 75% target remains open. Native non-Linux execution, live providers,
exhaustive workspace/all-serialized suites and performance benchmarks remain
outside local coverage.
