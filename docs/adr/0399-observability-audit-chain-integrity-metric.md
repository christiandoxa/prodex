# ADR 0399: Audit chain integrity metric

## Status

Accepted

## Context

Regulated deployments rely on append-only, tamper-evident audit chains for
security-sensitive actions. Operators need aggregate telemetry for audit append,
link verification, range verification, proof export, conflicts, digest failures,
and chain gaps without exposing tenant IDs, audit event IDs, digest material,
principal IDs, resource IDs, or storage internals in metric labels.

## Decision

Add `plan_audit_chain_metric` to `prodex-observability`.

The planner emits `prodex_audit_chain_events_total`, increments by one, and uses
only the closed enum labels `audit_chain_operation` and `audit_chain_result`.

## Consequences

- Audit storage and export adapters can publish integrity counters through one
  low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Tenant IDs, audit event IDs, digest values, chain topology, principal IDs,
  resource IDs, request payloads, and raw storage errors must stay out of metric
  labels.
