# ADR 0753: Redact domain audit query-plan error debug output

## Status

Accepted

## Context

`AuditQueryPlan` redacts tenant scope, time-range, and page-limit internals in
`Debug` output. Its error type wraps scope and timestamp validation branches.
The wrapper should keep redaction explicit instead of depending on derived
`Debug` behavior.

## Decision

Implement custom `Debug` for `AuditQueryPlanError`. Preserve `Scope`,
`Timestamp`, and `CursorSortOrderMismatch` branch names and delegate only to
nested domain errors with existing redaction contracts.

## Consequences

Diagnostics still show which query-plan validation branch failed. Tenant
identifiers and rejected timestamps remain out of formatter output.
