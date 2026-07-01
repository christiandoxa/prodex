# ADR 0507: Guard auth boundary unsafe-code prohibition

## Status

Accepted

## Context

Authentication and authorization are explicit enterprise security boundaries.
Both `prodex-authn` and `prodex-authz` already declare
`#![forbid(unsafe_code)]`, but the auth boundary guard did not enforce that
contract.

## Decision

The auth boundary guard now requires both auth boundary crate roots to keep
`#![forbid(unsafe_code)]`.

## Consequences

CI catches accidental removal of the auth boundary unsafe-code prohibition.
