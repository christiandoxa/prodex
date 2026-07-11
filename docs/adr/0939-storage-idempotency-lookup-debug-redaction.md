# ADR 0939: Storage Idempotency Lookup Debug Redaction

## Status

Accepted

## Context

Idempotency lookup storage plans carry tenant-scoped replay keys and request
fingerprints before mutating admin storage execution. Derived debug output
would expose those routing and replay values in diagnostics.

## Decision

Use custom `Debug` implementations for `IdempotencyRecordLookupCommand` and
`IdempotencyRecordLookupPlan`. Redact storage keys and operations while
preserving struct names.

## Consequences

Storage diagnostics can identify the lookup planner type without leaking tenant
identifiers, idempotency keys, or request fingerprints.
