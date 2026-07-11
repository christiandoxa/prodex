# 0225: Observability Trace Propagation Plan

## Status

Accepted.

## Context

The enterprise target requires end-to-end W3C trace context propagation. The
observability boundary already parses and renders `traceparent`, while HTTP
adapters preserve both `traceparent` and `tracestate` headers. Adapters still
need a reusable boundary plan that combines the canonical rendered traceparent
with a validated optional tracestate value.

Forwarding raw tracestate values without validation can allow empty or
control-character-bearing headers into downstream provider and telemetry
adapters.

## Decision

Add `plan_trace_propagation` to `prodex-observability`. The planner returns a
`TracePropagationPlan` with:

- canonical `traceparent` rendered from `TraceContext`;
- optional `tracestate` trimmed and rejected when empty, over 512 bytes, or
  containing non-printable ASCII.

The planner stays SDK-free and framework-free. Concrete adapters still own
actual OpenTelemetry SDK integration and header writes.

## Consequences

- Gateway and provider adapters can share one propagation contract.
- Invalid tracestate material fails before outbound propagation.
- Stable trace context error response planning continues to redact raw header
  values.

## Follow-up

ADR 0279 extends this propagation contract with validated optional W3C
`baggage`.
