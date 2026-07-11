# ADR 0390: Observability Role Binding Lifecycle Metric

## Status

Accepted.

## Context

Role binding grant and revoke operations are privilege-management control-plane mutations. Operators need to count authorization, denial, persistence, and failure outcomes without exposing tenant IDs, principal IDs, role names, role binding IDs, request payloads, caller identity, or raw storage errors as metric labels.

## Decision

Add `plan_role_binding_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_role_binding_lifecycle_events_total`, increments by one, and uses only the closed enum labels `role_binding_operation` and `role_binding_result`.

## Consequences

- Role binding adapters can publish grant, revoke, denial, persistence, and failure outcomes through one shared low-cardinality contract.
- Tenant IDs, principal IDs, role names, role binding IDs, request payloads, caller identity, and raw storage errors remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the role binding lifecycle telemetry contract before a concrete metrics backend is wired in.
