# Compiled cache freshness reused across quota and Codex update checks (2026-10-21)

## Production ownership

- `prodex-runtime-quota/src/cache.rs` now delegates its usage probe cache
  freshness predicate to the existing `runtime_probe_cache_freshness` owner
  in `prodex-runtime-state`, which is backed by the compiled
  `mojo/prodex_core/runtime_state_quota.mojo` policy.
- `prodex-update-notice/src/lib.rs` now invokes the existing typed
  `prodex_mojo_core::update_notice_policy::cache_is_fresh` operation
  when applying the persisted Codex version update-check TTL.
- Both native Rust `saturating_sub` freshness comparisons are removed.
  Neither caller computes a replacement result or silently falls back to
  an alternative Rust policy if the Mojo ABI reports an error.
- Rust keeps cache file serialization, timestamp acquisition, profile
  entries and domain enum materialization. All time arithmetic and
  strict/inclusive TTL comparisons are delegated to their owning
  compiled Mojo kernels without changing their existing boundary rules.
- Offline caller regressions cover future timestamps, exact TTL edge,
  negative TTL, and extrema `i64::MIN` / `i64::MAX`; Codex update
  cache tests use local fixture files only and perform no network calls.
- No counting-rule exemptions, new dependencies, release-version changes,
  or unrelated WIP changes.

## Validation at checkpoint

- `cargo test --locked -q -p prodex-runtime-quota`: 32 tests passed.
- `cargo test --locked -q -p prodex-update-notice`: 14 tests passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`: PASS.
- Cargo fmt, Mojo authority, no-fallback, size guard, audited production-share,
  documentation and diff-whitespace checks: PASS.
- Canonical Mojo share 36.15095049877659% -> 36.1502700871431%
  (134,449 Mojo / 237,468 Rust LOC): a small ratio decrease because
  the host boundary and tests grew while two duplicate decisions were
  eliminated. This does not complete the independent 75% goal.
