# 0423: Domain API metadata display redaction

## Status

Accepted

## Context

Pagination cursors and entity tags enter through HTTP request metadata. API
responses already redact invalid values, but generic error display text can be
logged before response planning.

## Decision

`CursorError::Display` and `EntityTagError::Display` no longer include rejected
input lengths.

## Consequences

- API metadata details stay out of generic error plumbing.
- Error response codes and messages remain unchanged.
