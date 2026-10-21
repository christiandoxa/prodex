# Legacy Caveman instruction paragraph selection owned by Mojo (2026-10-21)

## Production ownership

- **Consumer:** `crates/prodex-optional-tools/src/launch_home.rs` overlay
  configuration cleanup, in `remove_legacy_caveman_instructions`.
- **Compiled owner:** `mojo/prodex_core/optional_tools_policy.mojo` exports
  versioned `prodex_optional_tool_legacy_instruction_paragraphs_v1`,
  reached through
  `prodex-mojo-core/src/optional_tools_policy/legacy_paragraphs.rs`.
- Rust retains bounded TOML file reads, TOML table ownership, delimiter
  traversal over paragraph source bytes, crypto SHA-256 digest computation,
  and eventual file write. Mojo exclusively matches paragraph digests
  against the retired Caveman instruction's SHA-256 fingerprint, and
  returns input-order indices of only the paragraphs to retain.
- The previous Rust digest-to-hex string comparison, hard-coded expected
  digest and filter/retention decision were deleted; Mojo is the one
  production authority with no fallback on ABI failure. The kernel uses
  the existing compiled optional-tool hex-digit helper and never touches
  credential or filesystem state.
- The ABI bounds paragraph count, validates caller-owned digest arrays
  and output capacities, and reports the selected count. The Rust
  materializer validates all selected indices are in range, distinct and
  strictly increasing before applying them to the original TOML text.
- The historical separator `\n\n` is still preserved, and all unrelated
  developer instructions remain byte-for-byte unchanged.

## Offline regression and validation

- Direct Mojo ABI fixture confirms the exact 32-byte retired fingerprint
  is filtered out, a one-byte digest difference remains, errors cannot
  mutate outputs, and empty batches succeed.
- Rust bridge test verifies repeated legacy digests disappear while
  unrelated digest records retain stable order. Caller test verifies an
  ordinary three-paragraph UTF-8 instruction value is unchanged.
- `cargo test --locked -q -p prodex-optional-tools`: 68 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_legacy_paragraph_selection`:
  compiled bridge regression passed.
- Locked workspace Clippy all targets/features with `-D warnings`, Mojo
  no-fallback/ownership, source share, size guard, Cargo fmt, Markdown
  and diff-whitespace checks: PASS.
- Official audited share: 36.17645560046453% -> 36.17922036833963%
  (134,938 Mojo / 238,033 Rust production LOC). The project 75% target
  remains unmet; no counting rule or release metadata was changed.
