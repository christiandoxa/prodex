# ADR 0592: Gateway HTTP Trace Propagation Metric Plan

## Status

Accepted

## Context

The gateway HTTP boundary validates and preserves W3C trace context headers, and
`prodex-observability` already defines low-cardinality trace propagation
metrics. A concrete exporter should not have to inspect raw request headers to
decide which trace propagation counters to emit.

## Decision

Add trace propagation metric plans to `GatewayHttpPlan` and carry them into
`GatewayHttpResponsePlan`. The HTTP planner now records one closed-label metric
plan each for `traceparent`, `tracestate`, and `baggage`, marking the carrier
as propagated when the header is present and missing otherwise.

## Consequences

- Future gateway exporters can publish propagation counters from either HTTP
  request or response plans without parsing raw headers or adding
  tenant/request identifiers as labels.
- Request behavior is unchanged; trace context validation and header
  preservation remain handled by the existing HTTP boundary.
