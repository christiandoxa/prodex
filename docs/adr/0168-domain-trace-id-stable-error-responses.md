# 0168: Domain Trace ID Stable Error Responses

## Status

Accepted

## Context

Enterprise observability requires end-to-end correlation across request IDs,
call IDs, trace IDs, tenant context, and audit event IDs. Those values are
valid trace/log data after redaction policy, but they are high-cardinality and
must not become metric labels or client-visible validation details.

The domain `TraceId` parser rejects empty, overlong, and malformed IDs. Raw
validation errors can include length or shape details that are useful for
diagnostics but unnecessary in public API responses.

## Decision

`prodex-domain` owns `plan_trace_id_error_response`.

The planner maps trace ID validation failures to stable status/code/message
response plans. It preserves machine-readable distinction for missing trace IDs
while deliberately omitting trace values, lengths, request IDs, call IDs, tenant
IDs, audit event IDs, and traceparent internals.

## Consequences

- Gateway/control-plane adapters can reject malformed correlation inputs through
  one redacted domain boundary.
- Trace IDs remain usable for trusted logs/traces without becoming public error
  payload data.
- Observability boundary crates can continue adapting W3C trace-context errors
  without exposing raw correlation material.
