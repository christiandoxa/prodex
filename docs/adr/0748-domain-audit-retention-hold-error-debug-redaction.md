# ADR 0748: Redact domain audit retention-hold error debug output

## Status

Accepted

## Context

`AuditRetentionHold` debug output redacts tenant IDs, event IDs, reason codes,
and expiry timestamps. Its validation error wraps tenant-scope and timestamp
errors. The wrapper should keep the same redaction contract explicit so future
error variants do not accidentally expose held-event metadata.

## Decision

Implement custom `Debug` for `AuditRetentionHoldError`. Preserve `Scope` and
`Timestamp` variant shape while delegating only to already redacted nested
domain errors.

## Consequences

Diagnostics still identify whether legal-hold validation failed on tenant scope
or timestamp validation. Rejected timestamp values and tenant identifiers remain
out of formatter output.
