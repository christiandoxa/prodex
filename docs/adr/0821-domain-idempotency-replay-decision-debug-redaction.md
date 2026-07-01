# ADR 0821: Redact idempotency replay decision debug timestamps

Status: Accepted

## Context

`IdempotencyReplayDecision::AlreadyInProgress` returns the pending-start
timestamp needed by callers to handle duplicate in-flight mutations. Its debug
output exposed that raw timestamp through diagnostics.

## Decision

Redact `started_at_unix_ms` in the custom `Debug` implementation for
`IdempotencyReplayDecision::AlreadyInProgress` while preserving the decision
shape.

## Consequences

Callers still receive the pending timestamp through the typed decision value,
but diagnostics no longer expose it through replay-decision debug output.
