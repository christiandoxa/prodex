# ADR 0010: Runtime proxy request body limit

## Status

Accepted.

## Context

The gateway target requires explicit request body limits and bounded admission so
a client cannot force unbounded memory growth before routing, authentication, or
upstream provider calls. The current tiny-http capture path buffered the full
HTTP body before dispatching to the runtime proxy.

## Decision

Runtime HTTP request capture enforces a default 32 MiB body limit before a body is
buffered. Operators may override the limit with
`PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES` for compatibility or constrained
deployments. The override is an exact positive integer: empty,
whitespace-bearing, non-numeric, or zero values fail closed instead of falling
back to the default. Requests with a declared `Content-Length` above the limit,
or a body that exceeds the limit while streaming into the capture buffer, are
rejected with `413` before any provider request is attempted.

## Consequences

- Runtime gateway memory usage is bounded during request capture.
- Oversized requests fail locally and do not consume provider quota or virtual-key
  budget.
- WebSocket frames and upstream streaming semantics are unchanged; this ADR only
  covers initial HTTP request body capture.
