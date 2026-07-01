# ADR 0402: Identity context consistency metric

## Status

Accepted

## Context

Authentication, authorization, tenant resolution, and audit must use the same
canonical principal, tenant context, and correlation context. Operators need
aggregate telemetry for consistent contexts, missing principal or tenant data,
tenant mismatches, missing audit correlation, and failures without exposing
tenant IDs, principal IDs, request IDs, audit event IDs, trace IDs, raw claims,
or context payloads in metric labels.

## Decision

Add `plan_identity_context_metric` to `prodex-observability`.

The planner emits `prodex_identity_context_events_total`, increments by one, and
uses only the closed enum labels `identity_context_surface` and
`identity_context_result`.

## Consequences

- Authn, authz, data-plane, control-plane, and audit adapters can publish
  identity-context consistency counters through one low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Tenant IDs, principal IDs, request IDs, audit event IDs, trace IDs, raw
  context values, claim values, request payloads, and raw validation errors must
  stay out of metric labels.
