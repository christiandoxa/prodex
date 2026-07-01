# 0620. Domain Cursor Character Guard

## Status

Accepted

## Context

API pagination cursors are opaque request-controlled metadata. The domain
boundary already rejected empty and overlong values, but accepted whitespace,
control characters, and non-ASCII text.

That makes cursor handling more ambiguous across HTTP query parsing, logs,
storage predicates, and future OpenAPI examples.

## Decision

`Cursor::new` now accepts only ASCII graphic characters after trimming.
Whitespace, control characters, and non-ASCII values fail with
`CursorError::InvalidCharacter`.

The existing `plan_cursor_error_response` maps this to the same redacted
`pagination_cursor_invalid` response.

## Consequences

- Pagination cursors remain opaque but transport-stable.
- Raw invalid characters and offsets stay out of public API errors.
- Adapters still own decoding query parameters before passing cursor text to the
  domain boundary.
