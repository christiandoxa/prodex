# ADR 0505: Guard domain unsafe-code prohibition

## Status

Accepted

## Context

The enterprise quality gate requires core/domain code to forbid unsafe unless a
future exception is explicitly documented. `prodex-domain` already declares
`#![forbid(unsafe_code)]`, but the boundary guard did not enforce that contract.

## Decision

The domain boundary guard now requires `prodex-domain/src/lib.rs` to keep
`#![forbid(unsafe_code)]`.

## Consequences

CI catches removal of the domain unsafe-code prohibition before review.
