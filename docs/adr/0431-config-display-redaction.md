# ADR 0431: Redact config display identifiers

## Status

Accepted.

## Context

Config invalidation and publication response envelopes were stable, but local
`Display` output still included tenant and policy revision identifiers.

## Decision

Config invalidation and publication display text now reports the error class
without tenant IDs or revision IDs. Typed variants retain structured values for
matching and diagnostics that explicitly inspect them.

## Consequences

Config adapter logs and propagated errors avoid leaking tenant or revision
identifiers by default.
