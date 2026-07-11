# ADR 0382: Observability Trace Propagation Metric

## Status

Accepted.

## Context

Enterprise observability requires end-to-end W3C trace context propagation. Operators need to count propagated, rejected, and missing trace carriers without exposing trace IDs, span IDs, tenant baggage values, request IDs, or raw header values as metric labels.

## Decision

Add `plan_trace_propagation_metric` to `prodex-observability`.

The planner emits `prodex_trace_propagation_events_total`, increments by one, and uses only the closed enum labels `trace_carrier` and `trace_propagation_result`.

## Consequences

- Gateway and control-plane adapters can publish trace propagation outcomes through a shared low-cardinality contract.
- Trace IDs, span IDs, tenant baggage values, request IDs, and raw header values remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the trace propagation telemetry contract before a concrete metrics backend is wired in.
