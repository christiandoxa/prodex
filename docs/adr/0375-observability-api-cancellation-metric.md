# ADR 0375: Observability API Cancellation Metric

## Status

Accepted.

## Context

Enterprise HTTP data-plane hardening requires cancellation propagation across
client disconnects, timeout budgets, shutdown drain, provider streams, and
persistence work. Operators need to count cancellation sources without exposing
request IDs, tenant IDs, session IDs, route paths, or connection identifiers as
metric labels.

## Decision

Add `plan_api_cancellation_metric` to `prodex-observability`.

The planner emits `prodex_api_cancellation_events_total`, increments by one, and
uses only the closed enum labels `api_cancellation_surface` and
`api_cancellation_source`.

## Consequences

- Gateway and control-plane adapters can publish cancellation propagation
  outcomes through a shared low-cardinality contract.
- Request IDs, tenants, session IDs, routes, and connection identifiers remain
  trace/log/report data subject to redaction.
- The observability boundary guard can enforce the cancellation telemetry
  contract before a concrete metrics backend is wired in.
