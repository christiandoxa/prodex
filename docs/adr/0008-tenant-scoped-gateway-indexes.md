# ADR 0008: Gateway resources are indexed by tenant

## Status

Accepted.

## Context

Phase 0 audit identified tenant IDs that were optional and not consistently part
of database keys. Gateway virtual keys and SCIM users already carry tenant
metadata and admin-surface authorization checks, but the SQL schema did not have
tenant-scoped indexes for tenant-owned lookup paths.

The final enterprise design must make tenant context mandatory in multi-tenant
use cases and include tenant ID in predicates, foreign keys, unique constraints,
audit events, cache keys, and telemetry attributes.

## Decision

As a Phase 0 hardening step, gateway SQL schema bootstrap creates indexes for:

- `prodex_gateway_virtual_keys (tenant_id, name)`
- `prodex_gateway_scim_users (tenant_id, user_name)`

These indexes make tenant-scoped access paths explicit without changing the
existing API compatibility surface yet.

## Consequences

- Tenant-scoped admin list/get paths have database support for tenant predicates.
- Existing unscoped development data remains readable.
- A later migration must tighten nullability/uniqueness semantics for
  multi-tenant production mode and add PostgreSQL Row-Level Security.
