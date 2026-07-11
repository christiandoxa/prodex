# ADR 0516: Guard Redis storage plans against unsafe code

## Status

Accepted

## Context

`prodex-storage-redis` owns client-free Redis key and Lua script plans for distributed rate limiting and short-lived cache coordination. It already declares `#![forbid(unsafe_code)]`, but the Redis boundary guard did not enforce the crate-root invariant.

## Decision

The Redis storage boundary guard now requires `#![forbid(unsafe_code)]` in `crates/prodex-storage-redis/src/lib.rs`. Its self-test covers both the accepted crate root and a negative fixture without the attribute.

## Consequences

Redis storage planning remains unsafe-free while the guard continues to reject driver dependencies, whole-map JSON, and whole-list rewrites.
