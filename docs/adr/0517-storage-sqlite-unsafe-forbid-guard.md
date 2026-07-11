# ADR 0517: Guard SQLite storage plans against unsafe code

## Status

Accepted

## Context

`prodex-storage-sqlite` owns driver-free SQLite SQL plans for local compatibility and tests. It already declares `#![forbid(unsafe_code)]`, but the SQLite boundary guard did not enforce the crate-root invariant.

## Decision

The SQLite storage boundary guard now requires `#![forbid(unsafe_code)]` in `crates/prodex-storage-sqlite/src/lib.rs`. Its self-test covers both the accepted crate root and a negative fixture without the attribute.

## Consequences

SQLite storage planning remains unsafe-free while migration DDL stays explicitly planned outside request-serving paths.
