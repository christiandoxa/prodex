# ADR 0937: Storage idempotency pending debug redaction

## Status

Accepted.

## Context

Idempotency pending-record commands and plans carry tenant-scoped storage keys,
idempotency keys, request fingerprints, and start timestamps. These fields are
needed by storage adapters but should not appear in generic debug output.

## Decision

Use custom `Debug` implementations for `IdempotencyPendingRecordCommand` and
`IdempotencyPendingRecordPlan`. Redact storage keys, operations, entries, and
timestamps.

Regression coverage rejects tenant ID, idempotency key, fingerprint, and
timestamp values in rendered pending-record debug output.

## Consequences

Storage diagnostics can identify idempotency pending command/plan shape without
exposing replay keys or request fingerprints. Planning behavior is unchanged.
