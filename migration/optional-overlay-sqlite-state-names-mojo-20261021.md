# Codex overlay SQLite rollout-state filename classification in Mojo (2026-10-21)

## Production ownership

- **Consumers:** three paths in `crates/prodex-optional-tools/src/launch_home.rs`
  that share native Codex rollout-state SQLite files, localize overlay
  state, and selectively copy rollout-state symlinks from the base home.
- **Compiled owner:** `mojo/prodex_core/optional_tools_policy.mojo` exports
  `prodex_optional_tool_overlay_rollout_name_v1`. It reuses the exact
  suffix-matcher in compiled `core_file_policy.mojo`; the checked Rust
  adapter is `optional_tool_overlay_rollout_name` under
  `prodex-mojo-core/src/optional_tools_policy.rs`.
- Mojo exclusively decides whether a basename has the exact `state_`
  prefix and ends with exactly one of `.sqlite`, `.sqlite-shm`, or
  `.sqlite-wal`. Prefix/suffix matching is case-sensitive and must not
  accept additional filename extensions. The original native Rust
  `starts_with` plus suffix `any` implementation was deleted; there
  is no Rust classifier or feature-off recomputation.
- Rust retains filesystem enumeration, OsString-to-lossy-UTF8 filename
  projection (as in the prior caller), file removal/copying and symlink
  operations. No path writes or other host effects enter Mojo; the ABI
  validates bounded UTF-8 views before writing its single boolean.
- No change to credentials, networking, dependencies, release metadata
  or production-source counting rules.

## Regression and validation

- Direct Mojo ABI fixture covers accepted SQLite WAL names, wrong case,
  extraneous suffix, empty input, invalid ABI, null pointers, oversized
  input and output nonmutation on error.
- Core bridge test checks exact suffix/case/prefix conditions and Unicode
  names; optional-tools caller regression verifies all three managed
  suffixes and exclusions.
- `cargo test --locked -q -p prodex-optional-tools`: 67 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_overlay_rollout_state_name`:
  compiled bridge regression passed.
- Locked workspace Clippy all targets/features with `-D warnings`, Cargo
  formatting, Mojo no-fallback and authority guards, size guard, source
  share and documentation lint: PASS.
- Canonical source share: 36.17464732817056% -> 36.17645560046453%
  (134,885 Mojo / 237,968 Rust production LOC). The 75% target remains open.
