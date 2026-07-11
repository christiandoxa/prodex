# 0415: Gateway HTTP duplicate traceparent rejection

## Status

Accepted

## Context

Enterprise gateway routes require W3C trace context. If a request carries
multiple `traceparent` headers, proxies and HTTP frameworks may disagree on
which value wins, breaking end-to-end correlation.

## Decision

`prodex-gateway-http` now rejects duplicate `traceparent` headers before
planning the request. Duplicate trace context uses the same redacted
`invalid_trace_context` response as malformed or missing trace context.

## Consequences

- Ambiguous trace context fails closed at the HTTP boundary.
- Raw trace header values remain out of client-visible error messages.
