# ADR 0515: Guard PostgreSQL storage plans against unsafe code

## Status

Accepted

## Context

`prodex-storage-postgres` owns driver-free PostgreSQL SQL plans and migration text for tenant-scoped accounting. It already declares `#![forbid(unsafe_code)]`, but the boundary guard did not enforce that crate-root invariant.

## Decision

The PostgreSQL storage boundary guard now requires `#![forbid(unsafe_code)]` in `crates/prodex-storage-postgres/src/lib.rs`. Its self-test covers both the accepted crate root and a negative fixture without the attribute.

## Consequences

PostgreSQL storage planning remains unsafe-free unless this ADR and guard are deliberately revised.
