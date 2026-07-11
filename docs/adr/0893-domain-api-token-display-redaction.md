# ADR 0893: Domain API token display redaction

## Status

Accepted.

## Context

Pagination cursor and entity-tag response planners already expose stable
messages for malformed, empty, and oversized tokens. Raw `Display` output still
distinguished empty, too-long, and invalid-character failure classes.

## Decision

Render all `CursorError` variants as `pagination cursor is invalid` and all
`EntityTagError` variants as `entity tag is invalid`, matching their response
planners. Keep typed variants and response codes unchanged for trusted
diagnostics.

Regression coverage pins exact display strings and rejects empty, too-long,
length, and invalid-character details from raw display output.

## Consequences

API pagination and precondition boundaries can safely fall back to raw display
strings without exposing token validation shape. Diagnostics should continue
matching typed variants for exact reasons.
