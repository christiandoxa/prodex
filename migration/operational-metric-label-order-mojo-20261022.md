# Operational metric label ordering in Mojo (2026-10-22)

## Production ownership

Counter and histogram identity use a compiled key-then-value permutation.
Rust's tuple sort is removed. The existing operational-metrics owner reuses
the shared caller-owned index heapsort and UTF-8 byte comparator, with original
indices breaking exact ties. Duplicate labels stay separate and ordering
remains case-sensitive. No dependency, counting change or Rust fallback exists.

The host validates every `TelemetryAttribute` before submitting borrowed
non-secret views, materializes owned strings in the returned order, then
acquires the registry lock. Invalid labels or ABI failures drop the observation
before counter/histogram mutation. Empty label lists retain their existing
identity; repeated labels are not deduplicated.

The versioned ABI accepts at most 65,536 pairs and the existing telemetry
owner's 128-byte per-view limit. It validates every view, alignment, count and
capacity before output writes. Rust checks the returned complete permutation
without comparing or sorting its contents. The feature-local permutation
check keeps the observability-only bridge independent of the runtime-gated
file-policy adapter. Mojo retains no pointers or heap ownership.

## Verification

Strict Cargo sets `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default changed tests omit both variables and any archive override. Direct
Mojo assertions use `-D ASSERT=all`.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/operational_metric_label_order_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-observability --lib operational_metrics::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-observability --test operational_metrics`
- `cargo test --locked -q -p prodex-app --lib runtime_operational_metrics::label_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_operational_metrics:: -- --test-threads=1`
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
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

Direct Mojo, three strict order bridge tests, four existing histogram bridge
tests, three caller regressions before/after migration, all four registry and
11 observability-crate tests pass. Strict workspace Clippy, formatting,
ownership, authority, source-share, no-fallback, size, wildcard, secret and
staged churn checks pass. Changed tests pass in 5.12 seconds; their feature-off
zero-test harnesses do not replace strict positive bridge/caller coverage.
Documentation passes. Full default CI was last run on the immediately prior
structured-log checkpoint, not repeated for this focused metric change.

Evidence retains a new test wildcard import rejected by the cap; explicit
imports fixed it without changing the guard. The updated no-fallback check
also rejected the intentionally unchanged Rust sort during baseline testing,
then passed after the caller switched to Mojo. Canonical share is
36.403974353530636% (137,176 Mojo / 239,640 Rust production LOC). Checked host
view/permutation transport slightly lowers raw share despite transferring
semantic authority. The 75% target remains open. Native non-Linux execution,
live providers and exhaustive workspace/serialized suites remain outside
coverage.
