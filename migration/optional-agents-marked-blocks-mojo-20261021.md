# Optional-tools AGENTS.md marked-block stripping in compiled Mojo (2026-10-21)

## Production ownership

- **Consumer:** `crates/prodex-optional-tools/src/localization.rs`,
  `upsert_agents_block` and `remove_agents_block`, used to maintain managed
  Codex `AGENTS.md` and `AGENTS.override.md` instructions.
- **Compiled owner:** `mojo/prodex_core/optional_tools_policy.mojo`
  (`prodex_optional_tool_marked_block_strip_v1`) through the
  checked `prodex-mojo-core/src/optional_tools_policy/marked_blocks.rs`
  bridge.
- Mojo now owns the complete bounded UTF-8 marker search/strip algorithm:
  repeatedly find the first begin marker, find its first succeeding end
  marker, remove the entire inclusive span and **rescan the joined output**.
  A final orphan end is reported as missing begin. These details preserve
  the old `String::find` + `replace_range` behavior, even if an earlier
  deletion creates a new begin marker at its join seam.
- The former native Rust implementation (`find`, `replace_range`, missing
  marker checks, unmatched-end scan) was deleted without a Rust fallback.
  Rust retains file selection, symlink-safe host IO, error messaging,
  block insertion and UTF-8 string ownership. The host never recomputes
  Mojo's marker search when the ABI reports an error.
- An empty begin/end marker or any input over 1 MiB is rejected by the
  new checked ABI rather than risking an unbounded loop. Input pointers are
  borrowed for the call only; the result is caller-owned and revalidated
  as UTF-8 before returning to the host.

## Offline validation

- Direct Mojo ABI fixture: multiple blocks, Unicode, partial/orphan markers,
  rejected ABI/capacity and no result-length mutation on malformed input.
- Rust ABI regressions: repeated removals, Unicode boundaries, orphan
  detection, empty input, oversized text, and markers formed at the
  join seam of two formerly separated fragments.
- `cargo test --locked -q -p prodex-optional-tools`: 68 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_marked_blocks`:
  compiled bridge tests passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`: PASS.
- Cargo fmt, Mojo no-fallback and authority, source-share, size,
  documentation and whitespace checks: PASS.
- Clean-tree official source share: 36.18303260321851% ->
  36.19187448425159% (135,084 Mojo / 238,160 Rust LOC).
  The separate 75% project target remains unmet; no source-counting
  exemptions, dependencies, or release version were changed.
