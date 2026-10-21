# Super lexical workspace component normalization in Mojo (2026-10-21)

## Production ownership

- **Consumer:** `crates/prodex-app/src/runtime_tools/super_trust.rs`,
  `lexical_workspace_alias`, reached from trusted Super launch workspace
  planning and from managed profile-home deduplication.
- **Compiled owner:** `prodex_runtime_overlay_lexical_path_v1` in
  `mojo/prodex_core/runtime_overlay_policy.mojo`. The versioned, checked
  host bridge is `crates/prodex-mojo-core/src/runtime_overlay_policy/path_alias.rs`.
- Rust acquires platform-correct `std::path::Component` variants and retains
  exact original `OsStr` bytes. It only maps component kinds to five typed
  ABI tags (prefix, root, current, normal, parent) and applies returned
  source indices to `PathBuf`. No path text is sent across the ABI.
- Compiled Mojo owns every lexical stack decision: skipping `.`,
  cancelling preceding normal parts for `..`, retaining unresolved relative
  parent components, preventing ascent above a rooted or prefixed path,
  and preserving source indices. The former Rust component match
  decisions and mutation logic are deleted without any fallback.
- Mojo validates counts, tags and pointer capacities before writing
  selected original indices; Rust validates bounded, strictly increasing
  returned indices. Filesystem canonicalization and Windows prefix
  formatting remain host/OS responsibilities.
- No network, credentials, persistence, release changes, counting waivers,
  or new Rust lexical algorithm.

## Offline regression

- Direct Mojo ABI tests cover rooted and relative paths, repeated
  parent segments, ignored current-directory components, empty paths,
  invalid tags/ABI versions and output not modified on invalid input.
- Rust bridge tests cover selection and invalid tags.
- Production Super caller tests verify rooted/relative path aliases,
  Unicode-opaque non-UTF-8 Unix components, and interaction with
  previously migrated trust-entry deduplication.

## Validation at checkpoint

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_overlay_lexical_path_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_component_stack`: 1 compiled bridge test passed.
- `cargo test --locked -q -p prodex-app --lib runtime_tools::super_trust::tests::`:
  8 caller tests passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`:
  PASS with all compiled Mojo runtime objects.
- Mojo authority, no-fallback, source-share, size, Markdown, Cargo format,
  and source whitespace guards: PASS.
- Canonical share: 36.1587538766669% -> 36.16386756689226%
  (134,603 Mojo / 237,600 Rust first-party production LOC).
  The independent 75% project target remains outstanding.
