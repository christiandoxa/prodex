# Codex version probe and User-Agent sanitization in compiled Mojo (2026-10-21)

## Production ownership

- **Consumers:** `crates/prodex-app/src/quota_support/codex_openai_auth.rs`
  in the live Codex CLI version probe and OpenAI request User-Agent builder.
- **Compiled owner:** a focused
  `mojo/prodex_core/codex_auth_headers.mojo` production object, registered
  in `crates/prodex-mojo-core/build.rs` and accessed through a checked
  `crates/prodex-mojo-core/src/codex_auth_headers.rs` bridge.
- Mojo now selects the last whitespace-delimited version candidate whose
  first byte is an ASCII decimal digit and whose token contains a dot;
  it trims boundary punctuation according to the old ASCII
  alphanumeric/dot/hyphen rule. The Rust `split_whitespace`,
  `rev().find`, and `trim_matches` decision chain was deleted.
- Mojo also performs printable-ASCII User-Agent projection with one
  underscore per non-printable or non-ASCII Unicode scalar. The old
  Rust `chars().map` projection was deleted; no equivalent Rust sanitizer
  is used as a fallback.
- Rust retains operating-system/process execution, user-agent metadata
  composition, the mature HTTP HeaderValue validation dependency, and
  request construction. A valid Unicode HTTP header is returned without
  sanitization exactly as before; Mojo is called on the rejected-header
  path. Invalid Mojo ABI status fails closed rather than silently
  recomputing the projection in Rust.
- UTF-8 input is borrowed, max one MiB. The ABI validates version tags
  and pointer/capacity contracts; Rust verifies returned UTF-8 byte spans
  and output length.

## Offline regression and validation

- Direct compiled Mojo ABI fixture checks Unicode whitespace, rightmost
  dotted version, unsupported candidate, per-codepoint projection,
  malformed ABI version and insufficient output capacity.
- Two Rust compiled bridge tests cover prerelease values, punctuation,
  large input rejection, absence and Unicode scalar-count normalization.
- Two application caller tests verify deterministic version output and
  that the sanitizer activates only when HTTP HeaderValue rejects input.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_codex_`:
  passed.
- `cargo test --locked -q -p prodex-app --lib compiled_codex_`:
  two tests passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`:
  passed; Cargo fmt, Mojo no-fallback/authority, production share and
  size/docs guards also passed.
- Canonical broad share from 36.212821075505936% before this checkpoint
  to 36.22465766616466% on this worktree
  (135,419 Mojo / 238,412 Rust production LOC). The 75% target is
  independent and still not achieved.
