# ADR 0724: Redact domain audit query-scope debug output

## Status

Accepted

## Context

`AuditQueryScope` is the domain boundary that keeps audit queries tenant-scoped.
It is embedded in query, export, and retention plans. Derived `Debug` output
printed the raw tenant ID anywhere those plans were dumped in diagnostics.

## Decision

Implement custom `Debug` for `AuditQueryScope`. The scope remains serializable
and comparable with the original tenant ID, while debug output replaces the
tenant identifier with a redacted placeholder. Nested query-plan debug output
therefore inherits the redacted tenant representation.

## Consequences

- Cross-tenant authorization and query filtering are unchanged.
- Generic diagnostics for audit query/export/retention plans no longer expose
  tenant IDs through the shared scope field.
- Exact tenant context remains available through authorized audit/query paths.
