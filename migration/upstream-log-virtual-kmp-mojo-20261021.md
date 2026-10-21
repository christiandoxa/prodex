# Upstream log TUI virtual-field ASCII query in Mojo (2026-10-21)

## Production ownership

- **Consumer:** `crates/prodex-app/src/app_commands/log_upstream.rs` TUI
  live and historical upstream-payload event filtering.
- **Compiled owner:** `mojo/prodex_core/log_semantics.mojo` exports the
  versioned `prodex_mojo_log_virtual_fields_contains_v1` kernel, accessed
  through `prodex-mojo-core/src/log/virtual_fields_search.rs`.
- Mojo now owns ASCII-casefolded KMP substring matching over the *virtual*
  space-joined nine event fields. It searches across the original separators,
  adjacent empty fields and payload boundaries in O(n + m) time, using
  caller-owned O(query length) scratch. The old Rust `format!` concatenation
  of the potentially 128 KiB upstream body and other metadata was removed.
  No Rust fallback duplicates search or lowercases the long event payload.
- Rust retains scalar metadata conversions (`u64`, `usize`, `bool`), the
  original field order, event queue, query input and terminal rendering.
  The Mojo ABI borrows validated UTF-8 fields and query without retaining
  pointers, and bounds input lengths and scratch capacity before scanning.
- Exact byte-preserving ASCII-only folding means non-ASCII Unicode bytes
  remain case-sensitive, matching the existing log query contract.

## Offline validation

- Direct Mojo ABI test checks cross-field matches, repeated KMP patterns,
  empty search, invalid capacity, invalid ABI and rejected field length.
- Rust bridge tests check empty fields, query matches across separators,
  Unicode strings, long repeated payloads and large queries.
- App upstream TUI tests cover optional request IDs, numeric metadata,
  non-ASCII payload and normal search/scroll behavior.
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_virtual_fields_search_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-app --lib app_commands::log_upstream::tests::`: 3 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_virtual_log_search`:
  2 passed.
- Full locked workspace Clippy with `-D warnings`, Mojo authority, no-fallback
  guard, size, source-share, cargo format, Markdown and whitespace checks: PASS.
- Official share: 36.162710444266004% -> 36.17372536702826%
  (134,754 Mojo / 237,765 Rust production LOC). The 75% target is not met.
