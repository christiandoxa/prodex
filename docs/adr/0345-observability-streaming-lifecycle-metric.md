# ADR 0345: Observability Streaming Lifecycle Metric

## Status

Accepted.

## Context

Enterprise gateways must preserve streaming semantics and cancellation behavior.
Operators need lifecycle telemetry for completed, cancelled, interrupted, and
guardrail-blocked streams without labeling by tenant, request ID, call ID,
provider endpoint, model, prompt, or raw upstream error text.

## Decision

Add `plan_streaming_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_streaming_lifecycle_total` and
`prodex_streaming_lifecycle_duration_ms`, increments by one, stores duration as
a measurement value, and exposes only the closed enum labels
`stream_transport` and `stream_outcome`.

## Consequences

- Gateway adapters can publish streaming completion, cancellation, interruption,
  and guardrail lifecycle telemetry through one shared contract.
- Streaming labels stay low-cardinality and reviewable.
- Tenant IDs, request IDs, provider endpoints, models, prompts, and raw upstream
  error details remain trace/log concerns subject to redaction policy.
