# ADR 0725: Redact domain audit query-plan debug output

## Status

Accepted

## Context

Audit query and export plans combine tenant scope, time ranges, page limits, and
sort order. Stable error planners hide rejected query internals, but derived
`Debug` output still exposed raw timestamps and page sizes in generic
diagnostics. Export plans inherit the same query details.

## Decision

Implement custom `Debug` for `AuditQueryPlan` and `AuditExportPlan`. Query-plan
debug output keeps the redacted scope and sort order while redacting time-range
and page-limit internals. Export-plan debug output delegates to the redacted
query plan and keeps only the low-cardinality export format.

## Consequences

- Query filtering, pagination, export formatting, serialization, and equality
  are unchanged.
- Generic diagnostics for audit query and export planning no longer expose raw
  tenant IDs, timestamps, or page-limit internals.
- Exact query criteria remain available through authorized control-plane audit
  tooling.
