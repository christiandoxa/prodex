# ADR 0391: Observability Provider Credential Lifecycle Metric

## Status

Accepted.

## Context

Provider credential rotation is a security-sensitive control-plane mutation. Operators need to count rotation, reference validation, persistence, authorization, denial, and failure outcomes without exposing tenant IDs, provider credential IDs, provider account identifiers, secret references, request payloads, caller identity, or raw storage errors as metric labels.

## Decision

Add `plan_provider_credential_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_provider_credential_lifecycle_events_total`, increments by one, and uses only the closed enum labels `provider_credential_operation` and `provider_credential_result`.

## Consequences

- Provider credential rotation adapters can publish rotation, validation, persistence, denial, and failure outcomes through one shared low-cardinality contract.
- Tenant IDs, provider credential IDs, provider account identifiers, secret references, request payloads, caller identity, and raw storage errors remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the provider credential lifecycle telemetry contract before a concrete metrics backend is wired in.
