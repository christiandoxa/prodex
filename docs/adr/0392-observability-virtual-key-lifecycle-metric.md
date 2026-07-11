# ADR 0392: Observability Virtual Key Lifecycle Metric

## Status

Accepted.

## Context

Virtual key creation and secret rotation are security-sensitive control-plane key lifecycle operations. Operators need to count creation, rotation, secret-reference persistence, authorization, denial, and failure outcomes without exposing tenant IDs, virtual key IDs, key names, secret references, generated key material, request payloads, caller identity, or raw storage errors as metric labels.

## Decision

Add `plan_virtual_key_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_virtual_key_lifecycle_events_total`, increments by one, and uses only the closed enum labels `credential_lifecycle_operation` and `credential_lifecycle_result`.

## Consequences

- Virtual key lifecycle adapters can publish create, rotate-secret, persistence, denial, and failure outcomes through one shared low-cardinality contract.
- Tenant IDs, virtual key IDs, key names, secret references, generated key material, request payloads, caller identity, and raw storage errors remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the virtual key lifecycle telemetry contract before a concrete metrics backend is wired in.
