# ADR 0464: Auto redeem request IDs use UUIDv7

## Status

Accepted.

## Context

Runtime auto-redeem idempotency keys used process ID, wall-clock nanoseconds,
and a process-local counter. In multi-replica deployments that is weaker than
the typed globally unique identifier boundary and leaves collision avoidance to
local runtime identity.

## Decision

Generate runtime auto-redeem idempotency keys with the existing typed domain
`RequestId` UUIDv7 generator, keeping the `prodex-auto-redeem-` prefix for
compatibility with existing quota request payload expectations.

## Consequences

Runtime auto-redeem idempotency no longer depends on process-local counters or
timestamps while preserving existing request shape.
