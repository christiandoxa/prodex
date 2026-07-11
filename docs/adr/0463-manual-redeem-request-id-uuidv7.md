# ADR 0463: Manual redeem request IDs use UUIDv7

## Status

Accepted.

## Context

Manual quota reset-credit redemption used an idempotency key built from process
ID and wall-clock nanoseconds. That shape is local to one process and weaker
than the enterprise identifier boundary for multi-replica deployments.

## Decision

Generate manual redeem request IDs with the existing typed domain `RequestId`
UUIDv7 generator, keeping the `prodex-manual-redeem-` prefix for compatibility
with existing operator output and upstream idempotency payloads.

## Consequences

Manual redeem idempotency keys no longer depend on process-local identity while
remaining recognizable in CLI output.
