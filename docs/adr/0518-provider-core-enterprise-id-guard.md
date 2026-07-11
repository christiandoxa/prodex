# ADR 0518: Scan provider core for process-local identifiers

## Status

Accepted

## Context

`prodex-provider-core` defines provider IDs, model contracts, fallback behavior, and provider contract metadata used by gateway routing. The enterprise ID guard already rejects process-local `AtomicU64` and `.fetch_add(...)` generators in boundary crates, but provider core was not part of that scan.

## Decision

Add `prodex-provider-core` to the enterprise ID boundary guard and self-test that the crate remains in the scanned set.

## Consequences

Provider contract code cannot introduce process-local identifier generation without the CI guard failing. Provider routing stays aligned with the UUIDv7 typed-ID policy used by the rest of the enterprise boundary.
