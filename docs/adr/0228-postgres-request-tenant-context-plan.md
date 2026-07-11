# 0228: PostgreSQL Request Tenant Context Plan

## Status

Accepted.

## Context

PostgreSQL RLS policies use the `prodex.tenant_id` session setting as
defense-in-depth for tenant-owned tables. Query predicates still include
`tenant_id`, but RLS is only effective when request-serving adapters set the
tenant context before executing tenant-owned DML.

## Decision

Add `plan_postgres_tenant_context` and centralize request-path statement
construction in `prodex-storage-postgres` so every tenant-owned SQL plan starts
with:

```sql
SELECT set_config('prodex.tenant_id', $1, true)
```

The setting is local to the transaction scope selected by PostgreSQL for
`set_config(..., true)`. Request-serving adapters execute this before the
operation statement for reservation, reconciliation, audit, purge, and
idempotency plans.

## Consequences

Application authorization and tenant predicates remain primary controls. RLS
adds a database-layer guard against cross-tenant access if an adapter or query
regresses. New request-path Postgres plans must use the shared statement builder
or fail the boundary tests.
