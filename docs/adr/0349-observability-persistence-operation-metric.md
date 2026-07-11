# ADR 0349: Observability Persistence Operation Metric

## Status

Accepted.

## Context

Enterprise gateways and control-plane services need persistence failure
telemetry for reads, writes, commits, rollbacks, and health checks. These
metrics must not label by tenant, resource ID, storage key, backend endpoint,
query text, request ID, or raw storage error text.

## Decision

Add `plan_persistence_metric` to `prodex-observability`.

The planner emits `prodex_persistence_operations_total`, increments by one, and
exposes only the closed enum labels `persistence_operation` and
`persistence_result`.

## Consequences

- Storage adapters can publish persistence success, conflict, timeout,
  unavailable, and failure counters through one shared contract.
- Persistence labels remain low-cardinality and reviewable.
- Tenant IDs, resource IDs, storage keys, backend endpoints, queries, request
  IDs, and raw storage errors remain trace/log/audit concerns subject to
  redaction policy.
