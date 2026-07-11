# ADR 0346: Observability Audit Emission Metric

## Status

Accepted.

## Context

Regulated deployments need evidence that security-sensitive audit events are
emitted, persisted, exported, or dropped. Audit telemetry must not label by
tenant, audit event ID, principal, resource ID, storage error text, or backend
endpoint.

## Decision

Add `plan_audit_metric` to `prodex-observability`.

The planner emits `prodex_audit_events_total`, increments by one, and exposes
only the closed enum labels `audit_operation` and `audit_result`.

## Consequences

- Gateway and control-plane adapters can publish audit emission, persistence,
  export, and drop counters through one shared contract.
- Audit metric labels stay low-cardinality and reviewable.
- Tenant IDs, audit event IDs, principal/resource identifiers, and storage error
  details remain trace/log concerns subject to redaction policy.
