# ADR 0340: Observability Provider Latency and Error Metric

## Status

Accepted.

## Context

Enterprise operations require provider latency and error telemetry. Provider
metrics must support degradation alerting without labeling by tenant, model,
raw endpoint, credential, request ID, prompt, or upstream error text.

## Decision

Add `plan_provider_metric` to `prodex-observability`.

The planner emits `prodex_provider_requests_total` and
`prodex_provider_request_duration_ms`, increments the request counter by one,
stores duration as a measurement value, and exposes only the closed enum labels
`provider` and `provider_result`.

## Consequences

- Provider adapters can publish latency and error telemetry through one shared
  contract.
- Provider and result labels stay low-cardinality and reviewable.
- Model names, endpoints, credential references, tenants, request IDs, prompts,
  and upstream error text remain trace/log concerns subject to redaction policy.
