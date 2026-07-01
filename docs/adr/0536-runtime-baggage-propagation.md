# ADR 0536: Runtime compatibility transport forwards W3C baggage

## Status

Accepted

## Context

Enterprise trace propagation treats `traceparent`, `tracestate`, and W3C
`baggage` as the canonical context carriers. The gateway HTTP boundary already
preserves all three, but the local runtime compatibility transport only
forwarded `traceparent` and `tracestate` to OpenAI-compatible upstreams.

## Decision

Forward `baggage` through the same compatibility helper that already preserves
`traceparent` and `tracestate`.

## Consequences

Runtime compatibility requests now keep end-to-end trace context aligned with
the gateway HTTP boundary. Baggage values remain request context and must not be
copied into metric labels.
