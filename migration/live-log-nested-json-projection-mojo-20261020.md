# Nested Live-Log JSON Traversal and Clipping Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled live-log-record owner now owns nested JSON traversal,
JSON escaping, UTF-8-safe nested string clipping, and serialization through
its versioned, measured prodex_live_log_json_projection_v1 JSON-tree ABI.
The Rust host keeps Serde parsing and acquires a flat borrowed node structure,
with canonical Serde number spans stored separately from borrowed JSON text
and keys. Only the compiled Mojo kernel selects traversal order and clipping
boundaries, renders braces/commas/keys/scalars, and appends the truncation
marker to clipped strings. It is iterative rather than recursive and
allocates no Mojo runtime heap on either measure or write pass.

The previous Rust materialize_nested_string_clips recursive traversal,
UTF-8 clipping application, and full serde_json serialization of the
clipped tree were removed. The host still acquires selected
event/pid/timestamp Serde values for compact records and sends each to the
same compiled projection, followed by the existing compiled compact metadata
kernel. Serde value/number parsing and synchronized live-log effects
remain Rust responsibilities; no Rust semantic fallback was added.

## Verification

- prodex-runtime-log: all 32 tests pass, including two new end-to-end
  golden tests covering control-byte and Unicode escapes, signed and
  unsigned numeric lexemes, nested arrays/objects, exact 8 KiB UTF-8
  clipping and preservation of long object keys.
- prodex-mojo-core: three direct ABI tests pass, covering exact wire
  output, UTF-8 boundary clipping, 1024 levels of nested arrays, version
  and operation rejection, malformed tree rejection and bounded-output
  failures without corrupting caller metadata.
- Direct Mojo test verifies the measured two-pass JSON-tree ABI, actual
  output bytes, malformed parentage, insufficient capacity and versions.
- Strict no-fallback source guard/self-test, Cargo formatting, diff check
  and canonical production-share non-regression check pass.

The official production share increases from 35.80922552817385% to
35.82663759535591% (130,467 Mojo / 233,695 Rust production LOC).
The project target of 75% remains unmet. Counting definitions, release
floor and non-regression baseline were not changed. Remote CI was not
a prerequisite to the checkpoint.
