# 0190: Domain Audit Cursor Pagination

## Status

Accepted

## Context

Audit query and export APIs need cursor pagination that behaves the same way as
domain audit selection. `AuditQueryCursor` defines the versioned cursor shape,
but adapters still need one domain-owned rule for applying a cursor to ordered
results. If each adapter interprets cursor position independently, pagination
can skip or duplicate events, accept mismatched sort order, or silently bypass
tenant and timestamp checks.

## Decision

`AuditQueryPlan` owns `select_events_after`.

The helper validates that the supplied `AuditQueryCursor` uses the same
`AuditSortOrder` as the query plan, filters only events strictly after the
cursor position in the plan ordering, applies the existing tenant and timestamp
checks, sorts deterministically by timestamp and `AuditEventId`, and truncates
to `AuditPageLimit`.

`AuditExportPlan` delegates cursor-aware selection to its embedded
`AuditQueryPlan` so exports and interactive queries share the same pagination
contract.

## Consequences

- Audit pagination uses one deterministic domain ordering across query and
  export paths.
- Sort-order mismatches fail closed with the redacted audit cursor error
  envelope.
- Storage adapters can still push cursor predicates down for efficiency, but
  the domain boundary remains the compatibility contract.
