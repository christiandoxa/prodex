# Codex terminal User-Agent token sanitization and nonblank probes in Mojo (2026-10-21)

## Production owner

- **Consumer:** `crates/prodex-app/src/quota_support/codex_openai_auth.rs`
  for terminal version display, env probes and tmux fallbacks.
- **Compiled owner:** `mojo/prodex_core/codex_auth_headers.mojo` exports
  `prodex_codex_auth_terminal_token_ascii_v1` and
  `prodex_codex_auth_nonwhitespace_v1`, through typed checked
  `crates/prodex-mojo-core/src/codex_auth_headers.rs` adapters.
- A shared pure compiled character projector handles both the existing
  upstream-compatible User-Agent fallback and new terminal metadata.
  The terminal alphabet is ASCII alphanumeric plus hyphen, underscore,
  dot and slash. Every other Unicode scalar, including whitespace,
  is replaced with exactly one underscore.
- The Unicode trim/nonblank decision for TERM_PROGRAM, TERM and tmux
  environment values now uses the compiled owner; Rust retains the
  original String only if the Mojo predicate returns true.
- Rust's native `String::replace` character predicate and
  `value.trim().is_empty()` branch were deleted without Rust fallback.
  The Rust host continues reading environment variables, probing tmux
  and performing HTTP HeaderValue checks; Mojo never accesses process
  environment or sockets.
- ABI input byte limits, UTF-8 validation, output capacities and boolean
  tags are checked. Invalid Mojo output does not trigger a Rust copy of
  the former algorithm.

## Offline regression

- Direct Mojo test validates terminal scalar replacement, non-ASCII
  whitespace, blank/nonblank conditions, reuse of printable User-Agent
  projection and invalid ABI/capacity behavior.
- Rust bridge test verifies one underscore per invalid Unicode scalar
  and retention of exact values for valid terminal headers.
- App caller test checks Ghostty version token, blank environment
  suppression, preservation of original nonblank env strings, and
  terminal token projection.
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/codex_auth_terminal_token_test.mojo`:
  passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib compiled_terminal_token`:
  one passed.
- `cargo test --locked -q -p prodex-app --lib compiled_terminal_header_token`:
  one passed.
- Workspace Clippy with `-D warnings`, Mojo authority/no-fallback, size,
  source-share and Markdown guards pass.
- Canonical clean-worktree Mojo share: 36.246984427221506%
  (135,675 Mojo / 238,632 Rust production LOC). Target 75% remains
  unfulfilled; no source-counting rules or release metadata changed.
