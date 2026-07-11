# ADR 0101: PostgreSQL Append-Only Audit Plan

## Status

Accepted

## Context

Security-sensitive control-plane decisions produce an append-only audit write
plan, and the storage boundary models a tenant-scoped hash-chain audit append.
The PostgreSQL SQL-plan crate needs matching durable schema and request-path
DML while preserving the enterprise rule that DDL runs only through external
migrators.

## Decision

`prodex-storage-postgres` extends the initial tenant accounting migration with
`prodex_audit_log`. The table is tenant keyed, RLS protected, and includes
uniqueness constraints for the audit event ID, event digest, and previous
digest. Request-serving code receives only DML statements:

- `set_tenant_context`;
- `append_audit_hash_chain`.

`plan_postgres_append_only_audit` validates the tenant-scoped storage command
through the shared storage contract and rejects cross-tenant event/key
mismatches before exposing SQL statements.

## Consequences

- PostgreSQL becomes the durable append-only audit target for control-plane
  security events.
- Gateway or control-plane request paths still cannot plan DDL.
- Adapter implementations can execute the SQL plan transactionally while this
  crate remains driver-free and testable.
