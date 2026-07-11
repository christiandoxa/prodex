# ADR 0927: Storage audit retention purge debug redaction

## Status

Accepted.

## Context

Audit retention purge commands and plans carry tenant-scoped storage keys and
purge batches with tenant and audit event identifiers. Those fields are needed
by storage adapters but should not appear in generic debug output.

## Decision

Use custom `Debug` implementations for `AuditRetentionPurgeCommand` and
`AuditRetentionPurgePlan`. Redact storage keys and purge batches while keeping
the command/plan shape visible.

Regression coverage rejects tenant and audit event IDs in rendered purge debug
output.

## Consequences

Storage diagnostics can identify audit retention purge command/plan shape
without exposing purge predicates. Purge planning behavior is unchanged.
