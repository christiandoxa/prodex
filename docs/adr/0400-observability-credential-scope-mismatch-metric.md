# ADR 0400: Credential scope mismatch metric

## Status

Accepted

## Context

Enterprise deployments must reject data-plane credentials on control-plane
routes, control-plane credentials on data-plane inference or quota routes, and
break-glass credentials outside the explicit break-glass boundary. Operators
need aggregate telemetry for those route/credential misuse cases without
exposing tenant IDs, principal IDs, token material, route paths, credential IDs,
or request payloads in metric labels.

## Decision

Add `plan_credential_scope_mismatch_metric` to `prodex-observability`.

The planner emits `prodex_credential_scope_mismatch_events_total`, increments by
one, and uses only the closed enum labels `credential_scope_direction` and
`credential_scope_result`.

## Consequences

- Route-scoped authentication and authorization adapters can publish
  credential-scope mismatch counters through one low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Tenant IDs, principal IDs, token material, credential IDs, raw paths, route
  internals, request payloads, and raw authorization errors must stay out of
  metric labels.
