# ADR 0440: Redact control-plane authorization display values

## Status

Accepted.

## Context

Control-plane authorization response envelopes were stable, but local
`Display` output still included credential scopes, roles, resource kinds,
operation names, and break-glass timestamps.

## Decision

Control-plane authorization display text now reports only the error class.
Typed variants retain structured fields for matching and diagnostics that
explicitly inspect them.

## Consequences

Control-plane logs and adapter errors avoid leaking authorization internals by
default.
