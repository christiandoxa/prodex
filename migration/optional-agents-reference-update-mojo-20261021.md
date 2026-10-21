# Canonical managed AGENTS reference editing owned by compiled Mojo (2026-10-21)

## Production owner

- **Consumer:** `crates/prodex-optional-tools/src/localization.rs`
  `ensure_agents_reference`, used to keep one canonical `@SUB_AGENTS.md`
  reference in Codex `AGENTS.md` / `AGENTS.override.md`.
- **Compiled owner:** `mojo/prodex_core/optional_tools_policy.mojo`
  exports `prodex_optional_tool_agents_reference_update_v1` through
  `prodex-mojo-core/src/optional_tools_policy/agents_reference.rs`.
- Mojo now performs all text-line interpretation, Unicode whitespace
  trimming, exact reference equality, duplicate line suppression, CRLF
  conversion, empty-file normalization, final blank-line insertion and
  the idempotence decision. Rust's old `lines`, `trim`, `join`,
  `reference_seen` and `updated == contents` policy was deleted.
- Rust still acquires and localizes the effective file, converts the
  reference OS path to display UTF-8, and writes only if the compiled
  planner requests a write. Mojo borrows UTF-8 text and yields caller-owned
  normalized bytes, plus a checked `write_needed` flag. Neither side
  retries with a Rust fallback if the ABI fails.
- The original Rust `str::lines` semantics are preserved: one trailing
  empty line is omitted, terminal CR is stripped, and empty retained
  content produces `@reference\n`. Input/reference bounds and capacity
  are checked before output writes. The caller's second invocation is
  idempotent on an already-canonical file.
- No new dependency, changed source-counting rule, network, secrets or
  release metadata modification.

## Offline verification

- Direct Mojo ABI test: duplicate references, CRLF, output byte sequence,
  no-op on a canonical input, invalid ABI and capacity.
- Checked Rust bridge tests: repeated references, Unicode whitespace,
  empty output, and correct idempotent write-suppression.
- `cargo test --locked -q -p prodex-optional-tools`: 68 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_reference_update`:
  2 bridge tests passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`:
  PASS; Cargo formatting, Mojo no-fallback/authority, size guard,
  documentation and whitespace checks: PASS.
- Canonical clean-tree Mojo share 36.19187448425159% ->
  36.20172955427421% (135,174 Mojo / 238,217 Rust production LOC).
  The independent 75% target remains unmet.
