# ADR 0147: Trace context stable error responses

## Status

Accepted

## Context

`prodex-observability` validates W3C trace context before gateway and
application planning attach spans. Raw `TraceContextError` values can reveal
header parsing details, malformed traceparent structure, or span-id validation
internals. Gateway HTTP response planning previously owned a local copy of this
redaction decision.

## Decision

Add `plan_trace_context_error_response` to `prodex-observability`. It maps raw
trace-context validation failures to the stable `invalid_trace_context`
response.

Gateway HTTP planning now adapts that observability response for invalid
traceparent headers while retaining its HTTP-specific bad-request status.

## Consequences

Trace propagation validation has a single redaction boundary. HTTP adapters can
return stable client-visible trace context errors without inspecting raw
trace-context validation variants.
