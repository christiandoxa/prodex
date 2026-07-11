# ADR 0506: Guard gateway-core unsafe-code prohibition

## Status

Accepted

## Context

Gateway core is part of the enterprise core/domain boundary and should keep
unsafe code forbidden unless a future exception is explicitly documented.
`prodex-gateway-core` already declares `#![forbid(unsafe_code)]`, but its
boundary guard did not enforce that contract.

## Decision

The gateway-core boundary guard now requires
`crates/prodex-gateway-core/src/lib.rs` to keep `#![forbid(unsafe_code)]`.

## Consequences

CI catches accidental removal of the gateway core unsafe-code prohibition.
