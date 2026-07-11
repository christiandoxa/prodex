# ADR 0500: Use deterministic virtual-key metric hashes

## Status

Accepted

## Context

Gateway Prometheus metrics must not expose raw virtual-key names, tenant IDs, or
user IDs as labels. The metrics endpoint already emits a `key_hash` label
instead of the raw key name, but it used Rust's default hasher.

`DefaultHasher` is not a stable external contract, so label values could change
across toolchain implementations even when the virtual-key name did not.

## Decision

Gateway virtual-key metrics use a small in-place FNV-1a 64-bit hash for
`key_hash`. Raw tenant, user, and key values remain omitted from labels.

## Consequences

Operators get repeatable pseudonymous virtual-key labels without adding a new
dependency or exposing raw tenant-owned identifiers.
