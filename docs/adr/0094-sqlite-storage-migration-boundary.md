# ADR 0094: SQLite Storage Migration Boundary

## Status

Accepted

## Context

SQLite remains important for local development and backward-compatible gateway
state, but the enterprise target requires migrations to be versioned and kept out
of request-serving open paths. The current legacy gateway still creates and
alters SQLite tables during backend open, which couples request path availability
to schema mutation.

A small reversible step is to define the target SQLite contract in a driver-free
crate before wiring the existing adapter to it.

## Decision

Add `prodex-storage-sqlite` as a SQL planning boundary. It owns:

- explicit versioned migrations for local tenant accounting tables;
- tenant-scoped primary keys, unique constraints, and indexes;
- append-only usage ledger uniqueness by tenant, reservation, and event kind;
- `ExternalMigrator` migration planning;
- rejection of migration DDL from `GatewayRequestPath`;
- local atomic reservation DML wrapped by `BEGIN IMMEDIATE` and `COMMIT`.

Add `scripts/ci/storage-sqlite-boundary-guard.mjs` and include its self-test plus
workspace scan in preflight. The guard forbids SQLite drivers,
HTTP/runtime/network/filesystem dependencies, JSON persistence dependencies, and
other storage drivers in production dependencies and source. It permits only the
test-only `rusqlite` dev-dependency used by SQLite execution coverage.

## Consequences

Existing SQLite adapter migration can be performed incrementally without a
big-bang rewrite. SQLite remains a compatibility/local backend, while
PostgreSQL remains the production durable source of truth. Request-serving code
must not plan or execute DDL after it is migrated to this boundary.
