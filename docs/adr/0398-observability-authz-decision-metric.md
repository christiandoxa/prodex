# ADR 0398: Authorization decision metric

## Status

Accepted

## Context

Enterprise gateways and control planes must authorize every resource/action and
fail closed for credential-scope, role, tenant, and resource denials. Operators
need aggregate authorization decision telemetry across data-plane,
control-plane, billing, quota, and break-glass boundaries without exposing
tenant IDs, principals, roles, resource IDs, action names, policy internals, or
request payloads in metric labels.

## Decision

Add `plan_authz_decision_metric` to `prodex-observability`.

The planner emits `prodex_authz_decisions_total`, increments by one, and uses
only the closed enum labels `authz_boundary` and `authz_result`.

## Consequences

- Authz adapters can publish allowed and denied decisions through one
  low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Tenant IDs, principal IDs, role names, resource IDs, action names, policy
  revisions, credential internals, and raw authorization errors must stay out of
  metric labels.
