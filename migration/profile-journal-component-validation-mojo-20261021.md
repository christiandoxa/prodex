# Profile journal filename component and lifecycle verb policy in Mojo (2026-10-21)

## Production ownership

- **Consumer:** `crates/prodex-profile-export/src/import_journal.rs`,
  validating private import auth-update journals, profile lifecycle
  journals and names/tokens before writing files.
- **Compiled owner:** `mojo/prodex_core/profile_export_policy.mojo`,
  versioned `prodex_profile_export_journal_token_policy_v1`, through
  `crates/prodex-mojo-core/src/profile_export/journal_policy.rs`.
- Mojo solely decides whether a UTF-8 filename component consists
  of ASCII alphanumeric, dash, underscore or dot, excluding an empty
  string, `.` and `..`. It also alone recognizes the four exact
  lifecycle operation strings `import`, `login`, `manage`, `remove`.
  The old Rust character iterator, traversal/punctuation classifier
  and verb `matches!` were deleted. The caller trusts the checked
  Mojo boolean and never recomputes decisions in Rust.
- Rust still owns native path traversal checks, filesystem/file type
  access, journal serialization, limits on private file size, and
  the established error message strings. This kernel does not
  generate, access or delete any journal by itself.
- ABI inputs are borrowed validated UTF-8 with a 1 MiB per-token
  bound; an unknown output or ABI failure fails closed.

## Offline validation

- Direct Mojo ABI fixture checks valid identifiers, unsafe traversal,
  exact operation spelling, invalid operation/ABI and no output mutation
  on error.
- Rust bridge regression tests cover empty, dots, Unicode, slashes,
  backslashes, NUL and long inputs, plus all lifecycle operations.
- `cargo test --locked -q -p prodex-profile-export`: 60 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_journal_`:
  two compiled bridge tests passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`,
  Cargo fmt, Mojo no-fallback and authority, source-share, size
  and Markdown guards: PASS.
- Official clean-tree Mojo share: 36.20811037259316% ->
  36.212821075505936% (135,315 Mojo / 238,351 Rust production LOC).
  The independent 75% project goal remains unmet.
