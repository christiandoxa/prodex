# 0652: API Cursor and Entity Tag Exact Boundary

## Status

Accepted

## Context

`Cursor::new` and `EntityTag::new` trimmed input before validation and storage.
Both values are request-controlled API metadata used for pagination and
concurrency control, so silently accepting padded values can hide malformed
callers.

## Decision

`Cursor::new` and `EntityTag::new` now validate and store the exact provided
value. Empty strings remain empty errors, while whitespace-only, leading, or
trailing whitespace values are rejected as invalid characters.

## Consequences

- Pagination cursors and entity tags remain exact at the domain boundary.
- Existing valid cursor and entity-tag values keep working.
- Stable API error responses continue to redact rejected raw values and lengths.
