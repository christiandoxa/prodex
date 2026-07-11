# ADR 0509: Guard gateway-http unsafe-code prohibition

## Status

Accepted

## Context

`prodex-gateway-http` is the framework-neutral HTTP policy boundary. It already
declares `#![forbid(unsafe_code)]`, but the gateway HTTP boundary guard did not
enforce that crate-level contract.

## Decision

The gateway HTTP boundary guard now requires
`crates/prodex-gateway-http/src/lib.rs` to keep `#![forbid(unsafe_code)]`.

## Consequences

CI catches accidental removal of the gateway HTTP unsafe-code prohibition.
