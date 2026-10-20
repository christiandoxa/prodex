# Anthropic SSE Data-Line Selection Migrated to Mojo (2026-10-20)

## Production ownership

The compiled Anthropic stream policy now chooses the first exact
"data: " field from a transported SSE event through the versioned
prodex_anthropic_stream_data_line_v1 ABI. The kernel owns line
boundaries, case-sensitive header recognition, first-match selection,
and removal of precisely one trailing CR from each input line.

Rust retains String::from_utf8_lossy transport acquisition so malformed
UTF-8 continues to use its previous replacement-character behavior,
Serde decoding of the selected JSON, and response/event materialization.
The previous Rust lines/find_map/strip_prefix policy was deleted;
there is no Rust semantic fallback.

## Verification

- All 531 prodex-provider-core tests pass, including two new live
  Anthropic translator regressions for multi-line CRLF events,
  first-match precedence, strict data prefix, empty first value and
  malformed transport UTF-8 replacement behavior.
- Two direct core ABI tests verify UTF-8, first-match and one-CR
  semantics, malformed pointers/versions and unchanged output on
  rejection.
- Direct Mojo ABI test, no-fallback guard/self-test, Cargo formatting,
  and production-share non-regression validation pass.

Official share rises from the previous checkpoint's roughly 35.826%
to 35.87396734330007% (131,185 Mojo / 234,498 Rust production LOC).
This result includes earlier quota batch changes already checkpointed
on main. The 75% target remains unmet. No metric rules or historical
release floors changed. Remote CI green was not a prerequisite.
