# ADR 0355: Observability Provider Degradation Metric

## Status

Accepted.

## Context

Enterprise gateways need provider degradation telemetry for alerting on error
rate, latency, overload, transport failure, and open circuit conditions. These
metrics must not label by tenant ID, model, provider endpoint, credential name,
request ID, prompt, or raw upstream error text.

## Decision

Add `plan_provider_degradation_metric` to `prodex-observability`.

The planner emits `prodex_provider_degradation_events_total`, increments by one,
and exposes only the closed enum labels `provider`,
`provider_degradation_signal`, and `provider_degradation_severity`.

## Consequences

- Gateway adapters can publish provider degradation events through one shared
  contract for SLO alerting.
- Provider degradation labels remain low-cardinality and reviewable.
- Tenant IDs, models, provider endpoints, credential names, request IDs,
  prompts, and raw upstream errors remain trace/log/audit concerns subject to
  redaction policy.
