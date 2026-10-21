# Runtime broker log pagination parsing in compiled Mojo (2026-10-21)

## Production ownership

The existing `runtime_broker_admin.mojo` owner parses snapshot URL pagination
through `prodex_runtime_broker_snapshot_query_v1`. The production HTTP admin
adapter consumes its typed result and preserves the original status, code and
error message. Rust query splitting, key matching, numeric parsing, duplicate
precedence and limit decisions are deleted without fallback.

Mojo reuses `log_parse_u64_text` for checked unsigned integers. The parser keeps
first-question-mark semantics, ignored empty pairs, unknown keys with values,
case-sensitive names, last-valid-duplicate precedence and first-error order.
It deliberately performs neither URL decoding nor whitespace trim. Leading
plus/zeros and u64 maximum preserve Rust behavior; overflow and invalid digits
remain rejected. The caller supplies the unchanged maximum entry count.

The fixed-output v1 ABI validates version, input shape and UTF-8, positive
maximum and output alignment before writing. Work is linear in the borrowed
URL length and storage is constant; no new URL length restriction changes the
existing HTTP contract. Rust validates returned tags and numeric bounds and
retains HTTP authentication, response construction, log access and transport.

## Regression coverage

Direct Mojo and compiled Rust tests cover defaults, duplicates, unknown keys,
empty pairs, malformed pairs, Unicode names, numeric overflow, first-error
precedence, exact bounds, plus/zeros, malformed UTF-8 and atomic ABI rejection.
The HTTP caller test checks all four original JSON error messages/statuses and
the duplicate-key success result. The restored active authenticated live-log test verifies that unauthorized
requests cannot obtain snapshots.

## Validation commands

Use Mojo 1.1.0 and `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0` for direct
compiled Cargo checks:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_broker_snapshot_query_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib runtime_broker_admin::`
- `cargo test --locked -q -p prodex-app --lib runtime_broker::admin::tests -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_proxy_broker_live_log_snapshot_is_authenticated_and_bounded -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib 'main_internal_tests::runtime_proxy_' -- --test-threads=1`
- `node scripts/ci/test-serial.mjs --suite runtime-smoke`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/runtime-hotpath-guard.mjs --self-test`
- `node scripts/ci/runtime-hotpath-guard.mjs`
- `node scripts/ci/runtime-test-manifest-guard.mjs`
- `node scripts/compat/check-upstream-baseline.mjs`
- `node scripts/compat/capture-replay-fixture-tests.mjs`
- `npm run docs`
- `npm run test:changed`
- `git diff --check`

The changed-test runner retains its default feature selection; direct compiled
checks run separately. Live providers and native non-Linux execution remain
outside this offline checkpoint. The 75% project goal remains open.

Direct Mojo assertions, five strict compiled bridge tests, five HTTP adapter
tests, all 459 runtime proxy tests and strict workspace Clippy pass. The
no-fallback self-test, authority, source-share, size, crate-boundary and offline
compatibility gates pass. The default changed-test runner passes. Runtime-smoke
and the runtime manifest guard also pass.

The first post-rebase authenticated snapshot filters selected zero tests. The
legacy source test was not wired into the active harness, so it was moved into
the active broker test module, isolated and added to runtime-smoke. It now also
checks the real HTTP error status/body for invalid cursor and zero limit.
Zero-test filter runs are not counted as regression coverage.

Wiring the dormant test exposed its obsolete broker-key helper. The moved test
uses the current smart-context-aware key helper; the initial compile failure
is retained in the validation evidence and the corrected test is rerun.

Final validation on the rebased implementation passes: five compiled bridge
tests, five admin caller tests, the active authenticated HTTP test (one test,
with 403/400/200 checks), runtime-smoke including that test, the runtime manifest
guard (505 discovered tests), strict workspace Clippy and the changed-test
runner. The five shared runner self-tests also pass. The earlier full runtime
proxy run passed all 459 tests; the subsequent changes only restored test
wiring and merged the already-validated Codex-config checkpoint.

Canonical production share is 36.295778741969634% (136,103 Mojo / 238,880 Rust
production LOC). The 75% project target remains unmet.
