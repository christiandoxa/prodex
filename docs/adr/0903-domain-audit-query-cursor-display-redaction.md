# ADR 0903: Domain audit query-cursor display redaction

## Status

Accepted.

## Context

Audit query cursors encode pagination position, sort order, timestamps, and
event IDs. The response planner already returns one stable cursor message, but
raw `Display` output still distinguished malformed cursors, unsupported
versions, nested timestamp errors, and event-ID parse failures.

## Decision

Render `AuditQueryCursorError` with the same message used by
`plan_audit_query_cursor_error_response`. Keep typed variants and response
codes unchanged for trusted classification.

Regression coverage pins the exact display string and keeps existing debug
redaction assertions for rejected timestamp values.

## Consequences

Audit query and export pagination boundaries can safely fall back to raw
display strings without exposing cursor validation shape or cursor-position
metadata. Diagnostics should continue matching typed variants for exact
reasons.
