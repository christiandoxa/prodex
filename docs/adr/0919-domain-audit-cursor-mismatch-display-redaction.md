# ADR 0919: Domain audit cursor mismatch display redaction

## Status

Accepted.

## Context

Audit query and retention pagination reject cursors whose embedded sort order
does not match the requested sort order. Response planners already map those
failures to the stable `audit query cursor is invalid` message, but raw
`Display` formatting exposed the mismatch classification.

## Decision

Use the same redacted cursor message for
`AuditQueryPlanError::CursorSortOrderMismatch` and
`AuditRetentionPageError::CursorSortOrderMismatch` display output. Keep the
typed variants for planner classification and tests.

Regression coverage asserts the raw display strings and response plans stay
aligned.

## Consequences

Stringified domain audit errors no longer expose cursor-validation internals.
Cursor rejection codes and planner behavior remain unchanged.
