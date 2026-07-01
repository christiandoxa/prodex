# ADR 0102: SQLite Append-Only Audit Plan

## Status

Accepted

## Context

SQLite remains a local and compatibility backend, but enterprise audit
semantics must not disappear when running single-node or test deployments.
Security-sensitive control-plane decisions still need tenant-scoped,
append-only audit persistence, and request-serving SQLite paths must not run
migration DDL.

## Decision

`prodex-storage-sqlite` extends the explicit local migration with
`prodex_audit_log`. The table includes tenant-keyed primary and unique
constraints for audit event ID, event digest, and previous digest, plus a
tenant/time index for audit queries.

`plan_sqlite_append_only_audit` validates the shared storage audit command,
rejects cross-tenant event/key mismatches, and returns an explicit
`BEGIN IMMEDIATE` plus DML plus commit plan. DDL remains available only through
`plan_sqlite_migrations` in external migrator mode.

## Consequences

- Local compatibility deployments preserve immutable audit intent.
- SQLite request paths can append audit events without planning DDL.
- The crate remains driver-free and continues to reject database-driver,
  runtime, HTTP, and JSON-persistence dependencies.
