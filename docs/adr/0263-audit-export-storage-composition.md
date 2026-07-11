# ADR 0263: Audit Export Storage Composition

## Status

Accepted

## Context

The domain already owns audit query/export validation, pagination, sort order,
and redacted errors. Control-plane `AuditExport` authorization also exists.
The missing boundary was a storage/application composition plan for durable
audit export reads. Without it, adapters could reimplement tenant predicates,
time filters, and authorization/audit ordering around `prodex_audit_log`.

## Decision

Add `AuditExportQueryCommand` and `plan_audit_export_query` to
`prodex-storage`. The command wraps a validated `AuditExportPlan` and a tenant
storage key. It rejects storage/query tenant mismatches and records tenant ID,
sort order, and page limit for backend planners.

PostgreSQL and SQLite expose request-path SELECT plans for audit export. The
SQL predicates require `tenant_id`, optional time range bounds, stable
`occurred_at` ordering, and explicit limits. PostgreSQL continues to set the
RLS tenant context before the SELECT. No request-path DDL is introduced.

Add `plan_application_audit_export` to `prodex-application`. The planner
accepts only `AuditExport` on `ResourceKind::AuditLog`, verifies the action
tenant matches the query tenant, always plans append-only audit storage, and
only plans audit export query storage after an authorized decision.

## Consequences

Audit export now has a single application boundary for authorization,
immutable audit, and tenant-scoped query planning. Client-visible errors remain
redacted and do not expose tenant IDs, time bounds, cursor details, or SQL.
