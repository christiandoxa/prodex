# 0187: Domain Audit Export Plan

## Status

Accepted

## Context

Audit export adapters need to combine a tenant-scoped query contract with a
stable export format. If adapters carry those pieces separately, export
metadata can drift from the query authorization path and cross-tenant filtering
can be applied inconsistently across HTTP, control-plane, and storage code.

The domain already owns `AuditQueryPlan` and `AuditExportFormat`. It now needs
a small aggregate type for export use cases without introducing HTTP or storage
dependencies into `prodex-domain`.

## Decision

`prodex-domain` owns `AuditExportPlan`.

`AuditExportPlan` combines an `AuditQueryPlan` and an `AuditExportFormat`.
It exposes stable content type and file extension metadata by delegating to the
format, and it delegates event matching to `AuditQueryPlan` so tenant scoping
and timestamp validation fail closed before export serialization.

## Consequences

- Control-plane audit export code can use one domain contract for query
  filtering and response metadata.
- Export matching preserves the same cross-tenant and timestamp failure
  behavior as audit queries.
- Raw tenant IDs, event IDs, timestamps, resource IDs, and audit payload
  material remain out of export-facing errors by reusing existing query-plan
  and export-format response planners.
