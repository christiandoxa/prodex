# Super trusted-workspace deduplication and configuration budget in Mojo (2026-10-21)

## Production ownership and security

- **Consumer:** `crates/prodex-app/src/runtime_tools/super_trust.rs`, for
  direct and resumed Super launches and workspace trust propagation.
- **Compiled owner:** `mojo/prodex_core/runtime_overlay_policy.mojo` exports
  `prodex_runtime_overlay_trust_entry_plan_v1`, reached through
  `prodex-mojo-core/src/runtime_overlay_policy/trust_entries.rs`.
- Mojo exclusively chooses first exact path occurrences, stable original
  traversal order, serialization separator accounting, and the established
  `SUPER_TRUST_CONFIG_MAX_BYTES` truncation rule. It preserves the existing
  first-entry exception when the first encoded workspace exceeds the cap.
  The previous Rust `BTreeSet` deduplication and native length-based
  `break 'workspaces` decisions were deleted without a Rust policy fallback.
- The Mojo kernel uses shared caller-owned heap index sorting
  (`index_order_sort`) with deterministic original-index tie breaking,
  avoiding quadratic duplicate scans across many session workspaces.
- Rust retains OS-native path discovery, lexical/canonical path aliases,
  JSON string serialization for TOML-compatible key materialization,
  and child CLI argument construction. Paths are borrowed only during
  the ABI call. Mojo validates pointer alignment, UTF-8 path views,
  count and output capacity before writing results. Rust verifies
  returned indices are strictly increasing, bounded and unique.
- No credentials, secrets, arbitrary config values, provider network calls,
  or filesystem mutations cross the compiled decision boundary.

## Offline regression

- Direct Mojo ABI tests exercise first-seen de-duplication, byte-budget
  stop, first-entry exception, empty input, rejected ABI version,
  invalid capacity and malformed input.
- Rust bridge tests verify Unicode and case-sensitive exact paths,
  preserve input order, empty sets, and reject oversized path views.
- Runtime Super caller tests cover duplicate path elimination, original
  spelling, CLI suffix preservation and the 128 KiB limit with many paths.
- The 75% project target remains independent of CI's retained 7% floor;
  no baseline, exclusions or release version are modified.

## Validation at checkpoint

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_overlay_trust_entries_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_trust_selection`:
  two compiled bridge tests passed.
- `cargo test --locked -q -p prodex-app --lib runtime_tools::super_trust::tests::`:
  six caller tests passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`:
  PASS with the new compiled owner.
- Mojo authority, no-fallback, source-share, size, documentation, formatting
  and whitespace guards: PASS.
- Canonical production share: 36.1502700871431% -> 36.1587538766669%
  (134,546 Mojo / 237,552 Rust LOC), still below the independent 75% target.
