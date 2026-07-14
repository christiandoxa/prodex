# ADR 0011: SQLite Runtime Adapter Boundary

- Status: Accepted
- Scope: local governance persistence

## Context

`prodex-storage-sqlite` owns driver-free migration and SQL plans. Executing
governance lifecycle, approval, session, audit, and SIEM-outbox transactions
requires `rusqlite`, but adding that driver to the plan crate would mix storage
description with runtime I/O and make request-path DDL harder to prevent.

## Decision

Keep SQL and migration ownership in `prodex-storage-sqlite`. Put executing
SQLite governance repositories in `prodex-storage-sqlite-runtime`, depending
only on `prodex-domain`, storage contracts, `rusqlite`, and bounded serialization
and hashing support. The runtime adapter may execute previously planned
transactions, but it may not own policy semantics or run migrations while
serving requests. Composition code chooses when migrations run and injects the
ready repository.

SQLite remains the personal/local single-process authority. Enterprise and bank
profiles use PostgreSQL; Redis remains non-authoritative coordination.

## Consequences

The driver-free crate stays reusable and auditable. Runtime I/O has one explicit
dependency boundary, while tenant keys, optimistic concurrency, immutable
revisions, audit chaining, and outbox atomicity remain storage-contract
invariants. The extra crate is justified by preventing the SQL-plan layer from
acquiring execution and lifecycle responsibilities.

## Verification

`prodex-storage-sqlite-runtime` repository tests cover tenant isolation,
maker-checker lifecycle, activation/LKG rollback, session revocation, audit
integrity, and SIEM outbox retry/dead-letter behavior. Storage and SQLite
boundary guards reject policy ownership and request-path migration execution.
