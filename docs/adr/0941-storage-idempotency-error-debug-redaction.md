# ADR 0941: Storage Idempotency Error Debug Redaction

## Status

Accepted

## Context

Idempotency storage planner errors can carry tenant identifiers when storage
keys, operations, or lookup rows do not match. `Display` output is already
generic, but derived debug output would expose the mismatched tenant values.

## Decision

Use custom `Debug` implementations for idempotency pending-record,
completed-record, lookup-plan, and lookup-row errors. Redact tenant fields while
preserving the error variant names.

## Consequences

Storage diagnostics can distinguish idempotency error variants without leaking
tenant identifiers from mismatched replay storage boundaries.
