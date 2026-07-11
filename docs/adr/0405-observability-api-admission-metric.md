# ADR 0405: API admission metric

## Status

Accepted

## Context

Enterprise gateway and control-plane HTTP adapters need bounded admission
telemetry for accepted requests, global concurrency pressure, route-specific
limits, full local queues, and graceful draining. These counters must not expose
tenant IDs, request IDs, paths, connection IDs, raw capacities, queue depths, or
policy internals as metric labels.

## Decision

Add `plan_api_admission_metric` to `prodex-observability`.

The planner emits `prodex_api_admission_decisions_total`, increments by one,
and uses only the closed enum labels `api_admission_route` and
`api_admission_result`.

## Consequences

- Async HTTP adapters can publish bounded admission decisions through one
  low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Raw tenant, request, path, connection, capacity, queue-depth, and policy
  details must stay out of metric labels.
