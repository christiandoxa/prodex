# Profile journal filename parsing and failure classification in Mojo (2026-10-21)

## Production ownership

- **Consumers:** `crates/prodex-profile-export/src/import_journal.rs`,
  auth-update and lifecycle journal path validators.
- **Compiled owner:** `mojo/prodex_core/profile_export_policy.mojo`
  exports `prodex_profile_export_journal_filename_plan_v1`, through
  `crates/prodex-mojo-core/src/profile_export/journal_policy.rs`.
- Mojo now exclusively checks the exact lowercase `.json` suffix,
  finds the first `-` separating lifecycle operation from token,
  validates the operation and component through shared compiled
  predicates, and chooses the issue category. Rust's two old native
  `strip_suffix` parsers and `split_once` were deleted without any
  Rust fallback. Mojo also returns a checked byte span for an unknown
  operation so Rust can retain the original diagnostic text.
- Rust retains native `Path` component traversal checks (including
  `..` and OS-native non-UTF-8 paths), file I/O, serialization and
  stable user-facing messages for missing suffix, missing token,
  unsupported operation, and unsafe token. It materializes Mojo's
  issue enum but does not reimplement the decision.
- ABI is versioned and validates bounded UTF-8 input, pointer alignment,
  operation kind and output spans. The module keeps original case
  sensitivity and first-delimiter behavior; no CI counting rules,
  dependencies or release metadata changed.

## Offline validation

- Direct Mojo fixture exercises valid names, missing token, unsupported
  operation, unsafe token, malformed ABI operation and output
  nonmutation on error.
- Typed Rust bridge tests assert error categories, exact invalid
  operation spans, lowercase suffix requirements and multi-dash tokens.
- `cargo test --locked -q -p prodex-profile-export`: 60 tests passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_journal_filename_parsing`:
  compiled bridge test passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`:
  PASS; Cargo format, Mojo authority/no-fallback, source-share, size,
  documentation and whitespace checks: PASS.
- Audited clean-tree source ratio: 36.212821075505936% ->
  36.21645769744181% (135,384 Mojo / 238,435 Rust production LOC).
  The separate 75% objective is still not met.
