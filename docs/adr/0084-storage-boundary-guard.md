# ADR 0084: Storage boundary guard

## Status
Accepted

## Context
`prodex-storage` is an adapter-neutral contract crate for tenant-scoped storage
commands and validation. It must not grow concrete Postgres, SQLite, Redis,
filesystem, network, async-runtime, or HTTP behavior. Those concerns belong in
future adapter crates such as `prodex-storage-postgres`, `prodex-storage-redis`,
and `prodex-storage-sqlite`.

## Decision
Add `scripts/ci/storage-boundary-guard.mjs` and wire its self-test plus
workspace scan into npm scripts and local preflight. The guard requires
`prodex-storage` to depend only on
`prodex-domain`, forbids dev-dependencies and target-specific dependency
sections, and scans source files for forbidden filesystem, environment, process,
network, HTTP, database, transport, and async-runtime imports.

## Consequences
The storage contract can evolve independently from implementation crates while
preserving dependency inversion. Concrete adapters must implement the contract in
separate crates and keep migrations, connection pools, and blocking database
work outside the request-path contract layer.
