# ADR 0519: Forbid unsafe code in provider core

## Status

Accepted

## Context

`prodex-provider-core` defines provider IDs, model catalogs, fallback chains, and adapter contract metadata used by gateway routing. It is a focused enterprise boundary crate and should not need unsafe code.

## Decision

Declare `#![forbid(unsafe_code)]` at the provider-core crate root.

## Consequences

Provider contract and routing metadata remain safe Rust. If unsafe code is ever needed, it must be introduced through a deliberate ADR and crate-root policy change.
