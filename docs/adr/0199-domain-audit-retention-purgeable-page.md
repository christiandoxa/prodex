# 0199: Domain Audit Retention Purgeable Page

## Status

Accepted

## Context

`AuditRetentionPlan::purge_candidate_page` provides resumable retention cleanup
pagination, and `purgeable_candidates` excludes events protected by active legal
holds. Storage adapters need both properties together so large cleanup jobs can
resume with a cursor without reimplementing hold filtering outside the domain.

Duplicating that logic in storage backends risks deleting held audit events, or
letting tenant-scope errors escape the stable audit error envelope.

## Decision

`AuditRetentionPlan` owns `purgeable_candidate_page`.

The helper accepts events, legal holds, a bounded retention batch limit, and an
optional ascending audit cursor. It rejects descending cursors, skips retained or
hold-protected events, returns only purgeable events in oldest-first order, and
emits a next cursor when more purgeable events remain. Retention, hold,
timestamp, and cursor failures are routed through `AuditRetentionPageError` and
the existing redacted audit error planner.

## Consequences

- Retention cleanup can run as resumable pages without bypassing legal holds.
- Storage adapters receive one domain-owned page contract for purge deletes.
- Cross-tenant hold mistakes fail closed before storage deletion.
- Client-visible errors stay stable and redacted.
