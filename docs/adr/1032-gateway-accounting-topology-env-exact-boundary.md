# ADR 1032: Gateway accounting topology env exact boundary

## Status

Accepted.

## Context

`PRODEX_GATEWAY_REPLICA_COUNT` and
`PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS` declare deployment topology
intent for multi-replica accounting readiness. Runtime parsing previously
trim-normalized those values, allowing padded environment settings to activate
or alter the readiness gate differently from the exact secret/config value
operators supplied.

## Decision

Gateway accounting topology environment values are exact runtime inputs.
`PRODEX_GATEWAY_REPLICA_COUNT` must be non-empty, whitespace-free, and parse as
a positive integer. `PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS` must be
non-empty, whitespace-free, and one of the supported boolean literals.

## Consequences

Malformed deployment topology settings fail at startup instead of being
trim-normalized. Operators should set canonical values such as `3` and `true`
without padding.
