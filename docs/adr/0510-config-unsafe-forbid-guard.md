# ADR 0510: Guard config unsafe-code prohibition

## Status

Accepted

## Context

`prodex-config` owns revisioned configuration and cache decisions. It already
declares `#![forbid(unsafe_code)]`, but the config boundary guard did not enforce
that crate-level contract.

## Decision

The config boundary guard now requires `crates/prodex-config/src/lib.rs` to keep
`#![forbid(unsafe_code)]`.

## Consequences

CI catches accidental removal of the config boundary unsafe-code prohibition.
