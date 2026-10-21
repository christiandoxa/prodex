# Fixed proxy profile visibility in compiled Mojo (2026-10-21)

## Production ownership

- Consumer: `crates/prodex-runtime-launch/src/profile.rs`, called when
  building an isolated fixed-profile runtime proxy state.
- Compiled policy owner: `prodex_runtime_profile_exact_visibility_v1` in
  `mojo/prodex_core/profile_schedule.mojo` with a checked
  `prodex-mojo-core/src/runtime/profile_launch.rs` Rust FFI adapter.
- Rust clones caller-owned state, checks the selected profile exists,
  constructs borrowed views from four state collections, and applies the
  returned keep flags to the maps. Mojo owns every equality decision
  against the selected name (not prefix, not ASCII-casefold). The four
  original Rust comparison closures have been deleted, without fallback.
- One synchronous Mojo call generates masks for profiles, last-selected
  timestamps, response-profile bindings and session-profile bindings.
  Borrowed names are dropped before Rust mutates the cloned maps.
- Versioned Mojo ABI validates all borrowed UTF-8 names before writing
  outputs, with count, length, alignment, and capacity caps. Rust checks
  the returned per-entry boolean tags before applying them.

## Regression scope

- Direct Mojo ABI fixture tests exact Unicode matching, similar-prefix
  rejection, empty inputs, invalid ABI/capacity/input and untouched outputs.
- Rust bridge tests include Unicode case/prefix sensitivity and empty names.
- Runtime launch caller tests verify the original state is unchanged,
  bindings for other profiles are filtered across all four maps, and
  missing selected profiles are rejected.
- No change to persistence, credentials, runtime traffic, release-version
  metadata, or source-counting policy.

## Validation at checkpoint

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/profile_exact_visibility_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-runtime-launch`: 82 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core profile_visibility`:
  1 passed.
- `cargo test --locked -q -p prodex-app --lib runtime_launch::`:
  678 passed.
- Locked workspace Clippy (all targets/features, `-D warnings`): PASS.
- Mojo no-fallback, authority, size, production-share, Markdown and
  Cargo formatting/whitespace guards: PASS.
- Official Mojo share: 36.13600379134668% -> 36.13854323958409%
  (134,263 Mojo / 237,260 Rust production LOC). Project target 75% unmet.
