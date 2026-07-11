# ADR 0364: Observability API Deprecation Metric

## Status

Accepted.

## Context

Enterprise API governance requires a deprecation policy for versioned APIs.
Operators need to count deprecation notices, sunset warnings, and rejected use
without turning API versions, raw routes, tenants, client IDs, or request IDs
into metric labels.

## Decision

Add `plan_api_deprecation_metric` to `prodex-observability`.

The planner emits `prodex_api_deprecation_events_total`, increments by one, and
uses only the closed enum labels `api_deprecation_surface` and
`api_deprecation_signal`.

## Consequences

- Gateway and control-plane adapters can publish API lifecycle events through a
  shared low-cardinality contract.
- Raw routes, API versions, tenants, client identifiers, request IDs, and
  compatibility details remain trace/log/report data subject to redaction.
- Deprecation telemetry can be enforced by the observability boundary guard
  before a concrete metrics backend is wired in.
