# ADR 0895: Domain audit limit display redaction

## Status

Accepted.

## Context

Audit timestamp, page-limit, and retention-batch-limit response planners already
expose stable redacted messages. Raw `Display` output still distinguished zero,
future, too-large, and pre-Unix timestamp validation classes.

## Decision

Render `AuditTimestampError`, `AuditPageLimitError`, and
`AuditRetentionBatchLimitError` with the same messages used by their response
planners. Keep typed variants and response codes unchanged for trusted
diagnostics.

Regression coverage pins exact display strings and rejects future, too-large,
and raw limit values from display output.

## Consequences

Audit query and retention boundaries can safely fall back to raw display strings
without exposing timestamp or limit validation shape. Diagnostics should match
typed variants for exact reasons.
