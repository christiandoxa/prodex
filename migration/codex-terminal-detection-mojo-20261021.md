# Codex terminal name detection and tmux termtype tokenization in Mojo (2026-10-21)

## Production owner

- **Consumer:** `crates/prodex-app/src/quota_support/codex_openai_auth.rs`,
  used for the upstream-compatible Codex User-Agent terminal token.
- **Compiled owner:** `mojo/prodex_core/codex_auth_headers.mojo`,
  extending the existing versioned Codex-auth ABI with
  `prodex_codex_auth_terminal_name_v1` and
  `prodex_codex_auth_terminal_parts_v1`, through the checked typed
  `crates/prodex-mojo-core/src/codex_auth_headers.rs` adapter.
- Mojo now owns the whole case-insensitive `TERM_PROGRAM` alias decision:
  trim Unicode whitespace, delete ASCII interior space, hyphen,
  underscore and dot, fold ASCII case, and classify the normalized
  token into the 13 explicit supported terminal aliases or Unknown.
  Exact fallback `TERM` names (`dumb`, `wezterm`, `wezterm-mux`)
  also come from Mojo with strict case sensitivity.
- Mojo owns Unicode whitespace splitting of the first two tmux client
  termtype tokens and returns borrowed UTF-8 source spans. Original
  Rust `split_whitespace`, normalized String construction, alias
  `match`, and exact `TERM` match were deleted. The host only decodes
  checked enum tags into its existing display type.
- Rust retains process/env reads and tmux invocation, terminal
  information fallback order, socket interactions, header assembly
  and HTTP validation. Mojo never reads an environment variable and
  no Rust string normalizer acts as a fallback on Mojo ABI errors.
- All byte views are validated as UTF-8 under 1 MiB and all returned
  tags/ranges are checked; unknown terminal names remain supported.

## Offline regression

- Direct Mojo ABI test covers alias normalization, strict fallback,
  Unicode second token boundaries, invalid operation/ABI and null
  pointer errors.
- Rust compiled bridge tests cover Apple Terminal, Ghostty, iTerm2,
  Warp, Gnome, Windows Terminal and unknown Unicode aliases; exact
  `TERM` modes; empty and three-token tmux source inputs.
- Application caller test verifies original `TerminalName` mapping,
  tmux version tuple and `wezterm-mux` fallback.
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/codex_auth_terminal_policy_test.mojo`:
  passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib compiled_codex_`:
  four passed.
- `cargo test --locked -q -p prodex-app --lib compiled_codex_terminal`:
  one passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`:
  passed; Mojo ownership/no-fallback and source-share, Cargo fmt,
  size guard, Markdown and diff checks passed.
- Canonical Mojo share rises to 36.239431802392076% on the clean
  worktree (135,621 Mojo / 238,615 Rust production LOC). The separate
  75% target is not met.
