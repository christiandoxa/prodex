# 0626: Domain W3C Trace ID Guard

## Status

Accepted

## Context

Enterprise observability requires W3C trace-context compatibility. A W3C
`traceparent` trace ID is exactly 32 hexadecimal characters and must not be all
zero. The existing domain `TraceId::new` is intentionally compatibility-oriented
for older non-W3C correlation values, so tightening it would be a breaking
change.

## Decision

Add `TraceId::new_w3c`.

The strict constructor accepts only W3C-shaped trace IDs, lowercases the stored
value, and rejects all-zero IDs. Existing `TraceId::new` behavior remains
unchanged for legacy correlation sources.

## Consequences

- HTTP and observability adapters can enforce W3C trace-context at the domain
  boundary.
- Legacy correlation values keep backward compatibility.
- Public error response planning stays stable and redacted.
