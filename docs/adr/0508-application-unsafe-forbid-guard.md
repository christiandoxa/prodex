# ADR 0508: Guard application unsafe-code prohibition

## Status

Accepted

## Context

`prodex-application` is an enterprise use-case boundary. It already declares
`#![forbid(unsafe_code)]`, but the application boundary guard did not enforce
that crate-level contract.

## Decision

The application boundary guard now requires
`crates/prodex-application/src/lib.rs` to keep `#![forbid(unsafe_code)]`.

## Consequences

CI catches accidental removal of the application boundary unsafe-code
prohibition.
