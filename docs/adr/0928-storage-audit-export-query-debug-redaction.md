# ADR 0928: Storage audit export query debug redaction

## Status

Accepted.

## Context

Audit export query commands and plans carry tenant-scoped storage keys, audit
query plans, tenant IDs, time ranges, page limits, and export format metadata.
Those fields are needed by storage adapters but should not appear in generic
debug output.

## Decision

Use custom `Debug` implementations for `AuditExportQueryCommand` and
`AuditExportQueryPlan`. Redact storage keys, nested export/query payloads,
tenant IDs, and page limits while preserving low-cardinality sort order.

Regression coverage rejects tenant ID, timestamp, and page-limit values in
rendered audit export query debug output.

## Consequences

Storage diagnostics can identify audit export query shape without exposing
tenant or pagination details. Audit export planning behavior is unchanged.
