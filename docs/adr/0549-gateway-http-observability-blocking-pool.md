# ADR 0549: Send gateway HTTP observability on the blocking pool

## Status

Accepted

## Context

The legacy local rewrite gateway can export gateway spend events to an HTTP
observability endpoint. The sink used the blocking reqwest client inline while
emitting spend events, so a slow telemetry endpoint could delay the gateway
request path.

## Decision

Keep the existing HTTP payloads and authentication behavior, but schedule the
blocking HTTP send on the runtime blocking pool.

## Consequences

Telemetry endpoint latency no longer blocks the spend-event emitter. Failed
exports still produce the existing redacted runtime log event with request
sequence, endpoint, and error details.
