# Codex hook-trust preflight status and write-success policy in Mojo (2026-10-21)

## Production ownership

- **Consumers:** `crates/prodex-app/src/runtime_tools/hook_trust.rs`
  `hook_trust_updates`, `verify_hooks_trusted`, and the config/batchWrite
  status check in live Super hook-trust preflight.
- **Compiled owner:** `mojo/prodex_core/runtime_overlay_policy.mojo`
  versioned `prodex_runtime_overlay_hook_trust_policy_v1`, using a checked
  typed bridge `prodex-mojo-core/src/runtime_overlay_policy/hook_trust.rs`.
- Mojo exclusively classifies exact hook statuses `untrusted` and `modified`,
  returns first-seen ordered indices to approve or reject, and recognizes
  exact config success statuses `ok` and `okOverridden`. Rust's old
  `hook_needs_trust` and `matches!` status classifiers were deleted;
  no Rust fallback recomputes their decisions.
- Rust retains JSON-RPC subprocess and transport IO, per-entry shape/diagnostic
  validation, key and currentHash validation, error materialization, and
  trusted-hash edit construction. The Mojo ABI is read-only and does not
  send, redeem or authorize operations by itself.
- Batches preserve original order and exact case-sensitive matching. The
  checked bridge bounds status bytes/number of hooks, validates all returned
  indices are strictly increasing/in range, and rejects unknown outputs.
  Invalid Mojo policy responses do not silently mark a hook trustworthy.
- No dependency, release metadata, source-metric rule or external credentials
  are changed.

## Validation

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_overlay_hook_trust_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-app --lib runtime_tools::hook_trust::tests::`: 4 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_hook_`: 2 passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`: PASS.
- Mojo no-fallback, authority, size, official production-source share,
  Markdown and cargo fmt/diff checks: PASS.
- Official share 36.16211874128542% -> 36.162710444266004%
  (134,658 Mojo / 237,709 Rust production LOC). Target 75% remains unmet.
