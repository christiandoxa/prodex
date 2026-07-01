# 0203: SQL Audit Retention Purge Adapters

## Status

Accepted

## Context

`prodex-storage` defines an adapter-neutral retention purge plan. Concrete SQL
adapters still need DML-only plans that preserve tenant predicates and stay out
of migration/DDL paths.

The enterprise storage requirement is that tenant-owned deletes include
`tenant_id` in the predicate and that request-serving paths do not run schema
DDL.

## Decision

Add PostgreSQL and SQLite audit retention purge planners.

PostgreSQL emits `SET_TENANT_STATEMENT` followed by a scoped delete from
`prodex_audit_log` using `tenant_id = $1` and `audit_event_id = ANY($2)`.
SQLite emits an immediate transaction, a scoped delete using `tenant_id = ?1`
and `audit_event_id IN (?2)`, then commit. Both planners reuse
`plan_audit_retention_purge` so storage-key and batch tenant mismatches fail
before SQL is planned.

## Consequences

- Retention purge SQL is DML-only on request-serving paths.
- Concrete adapters inherit the storage boundary tenant validation.
- Deletes include tenant predicates and event ID filters.
- Adapter errors remain mapped to stable storage planning errors.
