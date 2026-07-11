# ADR 0938: Storage Idempotency Completed Debug Redaction

## Status

Accepted

## Context

Completed idempotency storage plans carry tenant-scoped replay keys, request
fingerprints, completion timestamps, and response bodies. Derived debug output
would expose those values in diagnostics.

## Decision

Use custom `Debug` implementations for `IdempotencyCompletedRecordCommand` and
`IdempotencyCompletedRecordPlan`. Redact storage keys, operations, completion
timestamps, response bodies, and replay entries while preserving struct names.

## Consequences

Storage diagnostics can identify the completed-record planner type without
leaking tenant identifiers, idempotency keys, request fingerprints, timestamps,
or completed response payloads.
