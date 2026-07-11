# ADR 0755: Redact domain audit query-cursor error debug output

## Status

Accepted

## Context

`AuditQueryCursor` redacts cursor position timestamp and audit event ID in
`Debug` output. Its parser error type wraps cursor parsing, sort order,
timestamp, and event ID validation errors. Cursor diagnostics should preserve
branch shape without leaking rejected cursor positions.

## Decision

Implement custom `Debug` for `AuditQueryCursorError`. Preserve branch names and
delegate only to nested domain errors with existing redaction contracts.

## Consequences

Diagnostics still show which cursor validation branch failed. Raw cursor
timestamps, event IDs, and opaque cursor payloads remain out of formatter
output.
