# Managed Super profile-home deduplication unified in compiled Mojo (2026-10-21)

- **Production consumer:** `super_trust_codex_homes` in
  `crates/prodex-app/src/runtime_tools/super_trust.rs`, used by resume
  workspace discovery and trusted Super launch argument construction.
- Previously, Rust selected first-seen lexical profile-home names with a
  `BTreeSet`. That decision is now delegated to the existing compiled
  `prodex_runtime_overlay_trust_entry_plan_v1` policy with an unbounded
  entry-byte budget because profile-home enumeration has no config output
  byte cap. The previous Rust set membership and retain loop were deleted.
- Rust still enumerates files, acquires paths and profiles, projects OS
  lexical path aliases through the already compiled Mojo component planner,
  and materializes original `PathBuf` values by checked selected indices.
  Mojo is the only authority for first-occurrence deduplication and source
  order, and there is no feature-off or error fallback in Rust.
- The new deterministic caller regression covers lexical aliases, duplicate
  paths, case-distinct paths, Unicode names, and empty collections.
- No credential changes, new dependencies, release-version change,
  source metric exemption, or unrelated local WIP files.

## Validation

- `cargo test --locked -q -p prodex-app --lib runtime_tools::super_trust::tests::`: 9 passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`: PASS.
- Cargo fmt, Mojo anti-fallback and ownership guards, source counting,
  size guard, docs lint and diff-whitespace checks: PASS.
- Clean-worktree audited Mojo source share: 36.16211874128542% before this
  checkpoint; the project target remains 75%.
