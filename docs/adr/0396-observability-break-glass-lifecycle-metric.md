# ADR 0396: Break-glass lifecycle metric

## Status

Accepted

## Context

Regulated deployments require break-glass access to be explicit, short-lived, and
audited. Operators also need aggregate telemetry for request, approval,
activation, revocation, expiry, denial, and failure paths without exposing
emergency-access identities or credential details in metric labels.

## Decision

Add `plan_break_glass_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_break_glass_lifecycle_events_total`, increments by
one, and uses only the closed enum labels `break_glass_operation` and
`break_glass_result`.

## Consequences

- Control-plane and authorization adapters can publish break-glass lifecycle
  counters through one low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Tenant IDs, principal IDs, token material, TTL values, request payloads, and
  raw storage or authorization errors must stay out of metric labels.
