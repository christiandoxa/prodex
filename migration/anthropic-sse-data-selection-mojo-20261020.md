# Anthropic Strict SSE Data-Line Selection Migrated to Mojo (2026-10-20)

## Production authority

The existing compiled anthropic_stream_policy.mojo owner now selects
the first strict data-field line for Anthropic Messages SSE translation.
The versioned prodex_anthropic_stream_data_line_v1 ABI scans ordered
lines, matches only a case-sensitive ASCII data colon space prefix,
strips the first six prefix bytes and removes exactly one trailing
carriage return, including the last line without a terminating LF.
A found but empty data field remains a found field and is still passed
to Serde parsing, so later valid data fields do not override it.

Rust retains transport String::from_utf8_lossy behavior (including
replacement of malformed UTF-8 bytes), ownership of the original
borrowed event text, Serde JSON decoding, and application of the
existing compiled Anthropic stream event-shaping kernels. The prior
Rust lines().find_map(strip_prefix) search was deleted rather than
kept as a semantic fallback. A typed adapter validates all output
bounds before exposing the borrowed field.

## Verification

- All 531 strict prodex-provider-core tests pass, including two
  new live translator regressions with CRLF, first-data precedence,
  exact prefix capitalization/spacing, empty first fields, end-of-file
  CR stripping, and malformed UTF-8 replacement.
- Two direct compiled-core ABI tests exercise Unicode, double CR,
  missing fields, strict matching, malformed pointers/UTF-8/versions,
  and unchanged result buffers on rejection.
- Direct Mojo ABI test, no-fallback guard/self-test, Cargo formatting,
  diff validation and canonical reachable-share check pass.

The official broad share increases from 35.868414646488134% to
35.87396734330007% (131,185 Mojo / 234,498 Rust production LOC).
The separate 75% project target remains unmet; counting definitions,
non-regression baseline and release floor remain unchanged. Remote CI
green is not a prerequisite for checkpoint publication.
