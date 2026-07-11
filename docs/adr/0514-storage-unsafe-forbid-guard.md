# ADR 0514: Guard storage boundary against unsafe code

## Status

Accepted

## Context

`prodex-storage` defines adapter-neutral storage contracts for tenant-scoped durable operations and multi-replica accounting evidence. It already declares `#![forbid(unsafe_code)]`, but the storage boundary guard did not enforce the crate-root invariant.

## Decision

The storage boundary guard now requires `#![forbid(unsafe_code)]` in `crates/prodex-storage/src/lib.rs`. Its self-test covers both the accepted contract fixture and a negative fixture without the attribute.

## Consequences

Storage boundary contracts remain free of unsafe escape hatches unless the guard and this ADR are deliberately revised.
