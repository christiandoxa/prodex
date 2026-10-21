# Persisted quota snapshot window reconstruction owned by Mojo (2026-10-21)

## Production ownership

- Consumer: `crates/prodex-runtime-quota/src/snapshot.rs`, used by cached
  quota source selection, profile rotation and application quota reports.
- Compiled owner: `prodex_runtime_quota_snapshot_rebuild_v1` in
  `mojo/prodex_core/runtime_state_quota.mojo`, reached through
  `prodex-mojo-core/src/runtime_state_quota/snapshot_rebuild.rs`.
- A versioned single ABI call chooses both five-hour and weekly usage-window
  presence, converts remaining percentage to a saturated used percentage,
  and interprets `i64::MAX` as an unknown reset timestamp.
- Rust now only projects the original `RuntimeQuotaWindowStatus` enum into
  fixed ABI tags and materializes `UsageWindow` values with the existing
  window durations. The Rust `Unknown` comparisons, arithmetic/clamping
  and reset-sentinel tests were deleted; there is no Rust fallback.
- The ABI rejects malformed status tags and capacities before output writes.
  The Rust adapter checks all flags, percent bounds and absence invariants
  before constructing the public domain model. Extreme snapshot percentages
  cannot overflow the usage-percentage conversion.

## Regression scope

- Direct Mojo ABI fixture: known/unknown status, clamp at i64 extrema,
  missing reset, invalid ABI/tag/capacity and untouched outputs.
- Rust bridge tests: known and unknown windows, reset sentinel, used-percent
  saturation and invalid status.
- Runtime quota caller tests: round-trip plan type, missing windows,
  weekly reset duration, and saturation for pathological persisted values.
- No live network fetch, no provider quota or redemption endpoint calls,
  no release version change or production source-counting exemption.

## Validation at checkpoint

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_quota_snapshot_rebuild_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-runtime-quota`: 31 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core snapshot_rebuild`:
  1 passed.
- `cargo test --locked -q -p prodex-app --lib quota_support::`:
  97 passed.
- Locked full workspace Clippy with `-D warnings`: PASS.
- Mojo anti-fallback, authority and size guards, Markdown lint,
  Cargo formatting, and production-share guards: PASS.
- Official share: 36.13854323958409% -> 36.139640888066104%
  (134,309 Mojo / 237,330 Rust production LOC). Target 75% unmet.
