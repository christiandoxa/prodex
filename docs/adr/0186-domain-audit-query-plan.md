# 0186: Domain Audit Query Plan

## Status

Accepted

## Context

Audit query and export adapters need to combine tenant scoping, time filtering,
page limits, and sort order. If each adapter assembles those pieces directly,
tenant isolation and bounded query behavior can drift across HTTP, control-plane
use cases, and storage adapters.

The domain already owns the individual validated pieces. It now needs a small
aggregate contract that adapters can pass around without reinterpreting raw
query parameters.

## Decision

`prodex-domain` owns `AuditQueryPlan` and
`plan_audit_query_plan_error_response`.

`AuditQueryPlan` combines `AuditQueryScope`, `AuditTimeRange`,
`AuditPageLimit`, and `AuditSortOrder`. `matches_event` fails closed for
cross-tenant events and invalid event timestamps, and otherwise reports whether
the event falls inside the validated time range. Response planning delegates to
the existing redacted audit scope and timestamp response planners.

## Consequences

- Gateway, control-plane, and storage adapters can share a single audit query
  contract.
- Tenant isolation is checked before time filtering.
- Raw tenant IDs, event IDs, timestamps, and audit payload material remain out
  of client-visible query-plan errors.
