# Managed AGENTS instruction-block final rendering in Mojo (2026-10-21)

## Ownership

- **Production consumers:** `crates/prodex-optional-tools/src/localization.rs`
  `upsert_agents_block` and `remove_agents_block`.
- **Compiled owner:** `mojo/prodex_core/optional_tools_policy.mojo` export
  `prodex_optional_tool_agents_block_render_v1`, with one checked host
  bridge at `crates/prodex-mojo-core/src/optional_tools_policy/agents_block_render.rs`.
- Mojo owns the final UTF-8 content for both insert and remove operations:
  Unix line separators, canonical blank line before a newly inserted
  block, Unicode trimming of the preserved text and inserted body, empty
  input handling, and exact remove-when-unchanged decision.
  The former Rust `cleaned.trim()`, `cleaned.trim_end()`, `block.trim()`,
  `format!` and `cleaned != contents` policy branches were removed.
- Rust obtains the original bounded file contents, calls the previously
  migrated Mojo marked-block strip, and applies the new Mojo
  `AgentsBlockRender` plan. File localization, native filesystem writes
  and effect ownership stay in Rust. The Rust bridge only validates ABI
  flags, result bounds and UTF-8; there is no semantic fallback.
- Mojo validates five borrowed UTF-8 views before output mutations,
  returns a stable output span and write/no-write tag, and respects
  caller-owned output capacity. No new dependency, release metadata or
  source counter exception was introduced.

## Validation

- Direct Mojo fixture: insertion byte-for-byte, idempotent removal,
  invalid operation/ABI, output buffer capacity and result nonmutation.
- Rust bridge: Unicode body trimming, separator placement, empty
  retained text, removal write-needed and no-op conditions.
- `cargo test --locked -q -p prodex-optional-tools`: 68 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_agents_block_`:
  2 bridge tests passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`,
  Mojo authority/no-fallback, size, audited production share, cargo format,
  documentation and diff checks: PASS.
- Canonical clean-worktree Mojo share: 36.20172955427421% ->
  36.20811037259316% (135,263 Mojo / 238,308 Rust production LOC).
  The long-term project goal remains 75%, not yet attained.
