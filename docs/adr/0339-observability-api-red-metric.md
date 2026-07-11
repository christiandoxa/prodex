# ADR 0339: Observability API RED Metric

## Status

Accepted.

## Context

Enterprise operations require RED metrics for API traffic: request rate, errors,
and duration. API telemetry must not label by raw URL path, tenant, user,
request ID, prompt, exact status code, or provider-specific endpoint.

## Decision

Add `plan_api_red_metric` to `prodex-observability`.

The planner emits `prodex_api_requests_total` and
`prodex_api_request_duration_ms`, increments the request counter by one, stores
duration as a measurement value, and exposes only the closed enum labels
`api_route` and `status_class`.

## Consequences

- API adapters can publish rate, error, and duration telemetry through one
  shared contract.
- Route and status labels stay low-cardinality and reviewable.
- Raw paths, tenant IDs, request IDs, prompts, and exact status codes remain
  trace/log concerns subject to redaction policy, not metric labels.
