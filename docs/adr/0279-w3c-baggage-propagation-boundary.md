# 0279: W3C Baggage Propagation Boundary

## Status

Accepted.

## Context

The enterprise observability target requires end-to-end context propagation, not
only trace identifiers. `prodex-observability` planned canonical `traceparent`
and optional `tracestate`, and `prodex-gateway-http` preserved those headers for
provider adapters. W3C `baggage` was still dropped at the gateway boundary, which
can break cross-service correlation for low-sensitivity routing and deployment
metadata.

Forwarding unbounded or control-character-bearing baggage would also risk
polluting downstream provider, log, or telemetry adapters.

## Decision

Extend `TracePropagationPlan` with optional validated `baggage`.
`plan_trace_propagation` trims baggage, rejects empty values, rejects values over
8192 bytes, and rejects non-printable characters. `prodex-gateway-http` now
preserves the normalized `baggage` header alongside `traceparent` and
`tracestate`, while continuing to strip auth and hop-by-hop headers.

The boundary remains SDK-free and framework-free. Concrete gateway and provider
adapters still own actual OpenTelemetry extraction/injection.

## Consequences

- Gateway/provider adapters can propagate W3C context without bespoke header
  allowlists.
- Invalid baggage is rejected before outbound propagation planning.
- Sensitive authentication headers remain excluded from upstream propagation.
