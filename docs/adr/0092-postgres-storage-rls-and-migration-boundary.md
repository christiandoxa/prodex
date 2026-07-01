# ADR 0092: PostgreSQL Storage RLS and Migration Boundary

## Status

Accepted

## Context

Enterprise Prodex requires PostgreSQL to be the durable source of truth for
multi-replica budget reservation, usage accounting, and append-only billing
ledger events. Tenant isolation must be enforced in application code and again
at the database layer. The existing local gateway backend still contains DDL in
open/init paths and legacy ledger uniqueness that is not tenant-scoped enough
for multi-replica production.

A safe incremental step is to introduce PostgreSQL SQL ownership separately from
request-serving adapters. This avoids a driver migration and a behavior change in
the same patch while making the target durable schema testable.

## Decision

Add `prodex-storage-postgres` as a driver-free SQL planning crate. It owns:

- versioned migration metadata for tenant accounting tables;
- tenant IDs in primary keys, unique constraints, and ledger uniqueness;
- Row-Level Security enablement and tenant policies using `prodex.tenant_id`;
- an explicit `ExternalMigrator` mode for DDL;
- rejection of DDL planning from `GatewayRequestPath`;
- a request-path atomic reservation DML plan that sets tenant context, upserts
  counters with conflict-safe arithmetic, inserts idempotent reservations, and
  appends ledger events with tenant-scoped uniqueness.

Add `scripts/ci/storage-postgres-boundary-guard.mjs` to keep the crate free from
PostgreSQL drivers, async runtimes, HTTP frameworks, filesystem/network/process
access, and other storage implementations. Adapter crates can execute these
plans later.

## Consequences

The schema and atomic SQL can now be tested independently of connection pools and
request handlers. Gateway code must not execute migrations on open/request paths;
production deployments should run these migrations through an external migrator
or control-plane rollout job. Later adapter work can wire this plan to a
PostgreSQL client and add multi-instance integration tests without redefining the
schema contract.
