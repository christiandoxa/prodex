# ADR 0377: Observability Provider Retry Metric

## Status

Accepted.

## Context

Enterprise provider adapters need safe retry telemetry for pre-commit retry attempts, retry denials after stream commit or cancellation, and retry-budget exhaustion. Operators need to count retry decisions without exposing retry keys, request IDs, endpoints, models, tenant IDs, retry-after values, or raw upstream error text as metric labels.

## Decision

Add `plan_provider_retry_metric` to `prodex-observability`.

The planner emits `prodex_provider_retry_events_total`, increments by one, and uses only the closed enum labels `provider`, `provider_retry_stage`, and `provider_retry_outcome`.

## Consequences

- Provider adapters can publish retry decisions through a shared low-cardinality contract.
- Retry keys, request IDs, endpoints, models, tenants, retry-after values, and raw upstream errors remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the provider retry telemetry contract before a concrete metrics backend is wired in.
