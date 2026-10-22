# Operational log detail rendering in Mojo (2026-10-22)

## Production ownership

The live log summary now calls `prodex_mojo_operational_log_details_v1` after
acquiring existing compiled metadata and host-prepared field text. Mojo owns
raw-field eligibility, format-specific blank/sentinel suppression, scalar
limits, labels, percent/ellipsis suffixes and event/detail separators.
Six Rust display helpers are deleted without a fallback. Their private helper
tests move to six stronger regressions through the real summary caller.

Rust retains the existing security redactor and mature URL parser, selecting
the appropriate acquisition boundary from the compiled format descriptor.
Absent input is acquired as an empty original string, matching the existing
eligibility rule. No new Rust field-selection table or rendering decision is
introduced. Existing public span/validation APIs remain compatible and their
pure prefix/control kernels are reused by the new renderer.

The versioned ABI uses caller-owned records with label/original/prepared UTF-8
views and an explicit format tag. It validates the 64-record plan bound,
64 MiB aggregate input/output limits, alignment, flags and every UTF-8 view
before output writes. A measured Rust allocation receives the result; short
capacity and malformed inputs leave output/length canaries unchanged. The
view record stays private to the independently compilable observability owner.

## Compatibility

Detail order and duplicate `path` labels remain intact. Plain empty/`-` prepared
values are omitted; percent values retain their distinct `%` and `-%` behavior.
Whitespace is preserved. C0/C1 original fields remain ineligible, while NBSP,
zero-width characters and BOM stay eligible. Ordinary/endpoint values retain
192 Unicode scalars; percent values retain 32. Scalar clipping can separate a
base from a combining mark, just as before. Endpoint query/fragment stripping,
secret redaction, TTFT versus latency labels and unknown-source behavior remain
covered. Neither upstream traffic nor persisted runtime state is changed.

## Verification

Focused Cargo checks use `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. Default changed tests omit both.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/operational_log_details_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib observability::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-observability --lib observability::`
- `cargo test --locked -q -p prodex-app --lib app_commands::log::log_stream::detail_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::log:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::log_tui -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/super-wildcard-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run docs`
- `npm run test:changed`
- `git diff --cached --check`
- `git diff --check`

The six caller regressions pass on the original implementation and again after
migration without expectation changes. Direct Mojo, 16 bridge tests in each
feature configuration, all 102 log tests and 17 TUI tests pass. Workspace
Clippy and formatting pass. Documentation and changed tests pass; feature-off
zero-test harnesses do not replace strict positive bridge/caller counts.

Canonical share is 36.38903569692801% (137,016 Mojo / 239,515 Rust production
LOC). Counting rules and guard limits are unchanged; the 75% target remains
open. Native non-Linux execution, live-provider traffic, full default CI and
exhaustive workspace/serialized runtime suites are outside this focused unit.
