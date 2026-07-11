# ADR 0443: Redact API version display values

## Status

Accepted.

## Context

API version response envelopes were stable, but local `Display` output still
included requested versions and sunset timestamps.

## Decision

API version display text now reports only the error class. Typed variants retain
structured fields for matching and diagnostics that explicitly inspect them.

## Consequences

API gateway logs and adapter errors avoid leaking requested version metadata or
sunset timestamps by default.
