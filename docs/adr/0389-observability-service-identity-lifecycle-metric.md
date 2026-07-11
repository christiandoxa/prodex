# ADR 0389: Observability Service Identity Lifecycle Metric

## Status

Accepted.

## Context

Enterprise control planes manage service identities separately from human users. Operators need to count service identity creation, secret rotation, disablement, authorization, persistence, denial, and failure outcomes without exposing tenant IDs, principal IDs, client IDs, service names, secret references, request payloads, or raw storage errors as metric labels.

## Decision

Add `plan_service_identity_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_service_identity_lifecycle_events_total`, increments by one, and uses only the closed enum labels `service_identity_operation` and `service_identity_result`.

## Consequences

- Service identity adapters can publish lifecycle, denial, persistence, and failure outcomes through one shared low-cardinality contract.
- Tenant IDs, principal IDs, client IDs, service names, secret references, request payloads, and raw storage errors remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the service identity lifecycle telemetry contract before a concrete metrics backend is wired in.
