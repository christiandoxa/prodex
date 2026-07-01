# ADR 0378: Observability Provider Circuit-Breaker Metric

## Status

Accepted.

## Context

Enterprise provider adapters need circuit-breaker telemetry for closed, open, and half-open probe decisions. Operators need to count circuit transitions and events without exposing retry-after values, endpoints, models, tenant IDs, request IDs, or raw upstream error text as metric labels.

## Decision

Add `plan_provider_circuit_breaker_metric` to `prodex-observability`.

The planner emits `prodex_provider_circuit_breaker_events_total`, increments by one, and uses only the closed enum labels `provider`, `provider_circuit_breaker_decision`, and `provider_circuit_breaker_event`.

## Consequences

- Provider adapters can publish circuit-breaker decisions through a shared low-cardinality contract.
- Retry-after values, endpoints, models, tenants, request IDs, and raw upstream errors remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the provider circuit-breaker telemetry contract before a concrete metrics backend is wired in.
