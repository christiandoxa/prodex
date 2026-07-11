# 0189: Domain Audit Query Cursor

## Status

Accepted

## Context

Audit query and export APIs need stable pagination semantics. Generic cursors
already exist in the domain API module, but audit adapters still need an
audit-specific cursor shape that records the event position used by
`AuditQueryPlan` ordering without depending on HTTP, storage, or filesystem
code.

If each adapter invents its own cursor format, pagination can drift across
control-plane query, export, and storage paths, and invalid cursor errors can
leak raw submitted cursor material.

## Decision

`prodex-domain` owns `AuditQueryCursor` and
`plan_audit_query_cursor_error_response`.

`AuditQueryCursor` stores the ordered audit position as an `AuditTimestamp`,
`AuditEventId`, and `AuditSortOrder`. It converts to and from the generic
`Cursor` boundary using a versioned `audit:v1` cursor payload. Parse failures
map to `AuditQueryCursorError`, and the response planner emits one stable,
redacted `audit_query_cursor_invalid` error.

## Consequences

- Audit query/export adapters can share one versioned cursor contract.
- Cursor parsing validates timestamp bounds, sort order, and event ID shape in
  the domain boundary.
- Raw cursor strings, event IDs, timestamps, and parser internals stay out of
  client-visible audit cursor errors.
