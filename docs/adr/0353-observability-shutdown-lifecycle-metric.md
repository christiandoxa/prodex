# ADR 0353: Observability Shutdown Lifecycle Metric

## Status

Accepted.

## Context

Enterprise gateways need graceful shutdown telemetry for signal receipt,
draining start, readiness disablement, in-flight drain completion, timeout, and
final completion. These metrics must not label by pod name, node name, signal
value, tenant ID, request ID, connection ID, profile, or raw shutdown error
text.

## Decision

Add `plan_shutdown_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_shutdown_lifecycle_total`, increments by one, and
exposes only the closed enum labels `shutdown_event` and `shutdown_result`.

## Consequences

- Gateway and control-plane adapters can publish shutdown and drain lifecycle
  counters through one shared contract.
- Shutdown labels remain low-cardinality and reviewable.
- Pod names, node names, signal values, tenant IDs, request IDs, connection IDs,
  profiles, and raw shutdown errors remain trace/log/audit concerns subject to
  redaction policy.
