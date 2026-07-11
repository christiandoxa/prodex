# 0238: Gateway HTTP Timeout Ordering

## Status

Accepted

## Context

Gateway HTTP adapters need explicit timeout budgets for request execution,
stream idle detection, and graceful drain. A non-zero budget is not sufficient:
if the stream-idle timeout is longer than the request timeout, a concrete async
adapter can apply inconsistent cancellation and backpressure behavior.

## Decision

`prodex-gateway-http` rejects HTTP policies where
`stream_idle_timeout_ms > request_timeout_ms`.

The failure maps to the existing redacted gateway HTTP policy response:
`gateway_http_policy_invalid`.

## Consequences

- Async HTTP adapters receive coherent timeout budgets before serving traffic.
- Misconfigured timeout policies fail before request execution.
- Client-visible responses do not expose internal timeout values or policy
  variant names.
