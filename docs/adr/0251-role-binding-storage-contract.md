# ADR 0251: Role-Binding Storage Contract

## Status

Accepted

## Context

Role and permission management must be durable, tenant-scoped, and safe across
replicas. The control-plane authorization boundary now exposes explicit
role-binding grant and revoke operations, but adapters still need a shared
storage contract so role changes do not become local in-memory state or
adapter-specific user metadata.

## Decision

Introduce a typed `RoleBindingId` and an adapter-neutral
`RoleBindingMutationCommand` in the storage boundary. PostgreSQL and SQLite
plans persist role bindings in tenant-owned tables with `(tenant_id,
role_binding_id)` primary keys and `(tenant_id, principal_id, role_name)`
uniqueness. Request-serving plans emit DML only:

- grant uses a tenant-scoped upsert;
- revoke uses a tenant-scoped update with `revoked_at_unix_ms IS NULL`;
- PostgreSQL request paths start by setting the tenant RLS context;
- SQLite local compatibility paths use `BEGIN IMMEDIATE` around the mutation.

## Consequences

Control-plane role changes can use the same tenant-scoped durable storage model
as audit and idempotency. Cross-tenant storage keys are rejected before adapter
execution, and migration DDL remains in explicit migration plans rather than
request-serving open paths.
