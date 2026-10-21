# Log-load aggregate query reuse of compiled virtual field search (2026-10-21)

## Production ownership

- **Consumer:** `crates/prodex-app/src/app_commands/log_load.rs`
  aggregate search for the `log stream` TUI.
- **Compiled owner:** The existing versioned O(n + m) KMP
  `prodex_mojo_log_virtual_fields_contains_v1` policy in
  `mojo/prodex_core/log_semantics.mojo`.
- The compiled ABI now accepts one to nine borrowed, space-delimited UTF-8
  fields with the original separator semantics, rather than being restricted
  to exactly nine upstream-payload fields. The checked bridge uses a fixed
  nine-view stack array and passes the actual field count. No Rust search
  fallback or concatenated `format!` string is retained.
- `LogLoadAggregate::matches` passes timestamp, source and transcript text
  as three borrowed fields; it still checks individual unique run IDs with
  the separately compiled ASCII-folded log search. No change is made to
  aggregate decisions, KMP folding rules, the cross-field query behavior,
  redaction policy, log-file IO, or release metadata.
- The ABI validates the 1-9 field count, lengths, scratch capacity and
  UTF-8 views before scanning. Rune/Unicode bytes remain exact under
  ASCII-only casefolding.

## Validation

- Direct Mojo ABI fixture checks three-field cross-boundary matches,
  malformed zero-count inputs, nine-field compatibility and invalid shapes.
- Rust bridge tests cover three- and nine-field matching, count bounds,
  long repetitive payloads and empty fields.
- App regression validates exact boundary spaces, aggregate search and
  separate run-ID matching.
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_virtual_fields_search_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_virtual_log_search`:
  3 tests passed.
- `cargo test --locked -q -p prodex-app --lib app_commands::log_`: 53 passed.
- `cargo test --locked -q -p prodex-app --lib compiled_log_load_search`: 1 passed.
- Workspace Clippy with `-D warnings`, Mojo authority/no-fallback, size,
  docs, format and production-source checks: PASS.
- Audited production share 36.17480355532655% -> 36.17422107720795%
  (134,796 Mojo / 237,834 Rust LOC). This small numerical decrease
  reflects a larger checked bridge/test surface; the duplicated Rust
  joined-string search has been removed. The 75% target is still open.
