# ADR 0820: Redact idempotency entry debug timestamps

Status: Accepted

## Context

`IdempotencyEntry::Pending` carries the operation and pending-start timestamp
used to guard duplicate mutations. Its debug output already redacted operation
identity material but still exposed the raw pending timestamp.

## Decision

Redact `started_at_unix_ms` in the custom `Debug` implementation for
`IdempotencyEntry::Pending` while preserving the pending/completed shape.

## Consequences

Diagnostics can still distinguish pending idempotency records from completed
records, but pending start timestamps no longer appear through entry debug
output.
