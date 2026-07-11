# ADR 0731: Redact domain audit query cursor debug output

## Status

Accepted

## Context

Audit query cursors encode an audit event position for pagination. The exact
timestamp and event ID must remain available in the opaque cursor string for
stable pagination, but generic `Debug` output can surface through panic
diagnostics or structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditQueryCursor`. Debug output redacts the
timestamp and event ID while preserving the sort order. Cursor serialization,
parsing, ordering, and pagination semantics are unchanged.

## Consequences

- Audit pagination remains backward compatible.
- Generic diagnostics no longer expose audit cursor position timestamps or
  audit event IDs.
- Exact cursor values remain available through the existing opaque cursor
  serialization path.
