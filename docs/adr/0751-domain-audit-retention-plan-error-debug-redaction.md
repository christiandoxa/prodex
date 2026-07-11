# ADR 0751: Redact domain audit retention-plan error debug output

## Status

Accepted

## Context

`AuditRetentionPlan` redacts tenant scope, retention policy, and timestamps in
`Debug` output. Its error type wraps tenant-scope and timestamp validation
errors. The wrapper should keep that formatter contract explicit so retention
cleanup diagnostics do not depend on derived `Debug` behavior.

## Decision

Implement custom `Debug` for `AuditRetentionPlanError`. Preserve `Scope` and
`Timestamp` variant shape and delegate only to nested domain errors with
existing redaction contracts.

## Consequences

Diagnostics still show which validation branch failed. Tenant identifiers and
rejected timestamps remain out of formatter output.
