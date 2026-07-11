# ADR 0890: Domain migration validation display redaction

## Status

Accepted.

## Context

Migration version and compatibility response planners already expose stable
messages. Raw `Display` output still distinguished empty versions, invalid
characters, compatibility-window cardinality, and unsupported source-window
details.

## Decision

Render `MigrationVersionError` as `migration version is invalid` and render
`MigrationCompatibilityError` variants with the same messages used by their
response planners. Keep typed variants and response codes unchanged for trusted
classification.

Regression coverage pins exact display strings and rejects empty, cardinality,
and window-detail wording from raw display output.

## Consequences

Migration validation boundaries can safely fall back to raw display strings
without exposing rejected input shape or compatibility-window internals.
Diagnostics should continue matching typed variants when exact reasons matter.
