# ADR 0441: Redact provider route display values

## Status

Accepted.

## Context

Provider route response envelopes were stable, but local `Display` output still
included rejected model length.

## Decision

Provider route display text now reports the error class without rejected input
details. Typed variants retain structured fields for matching and diagnostics
that explicitly inspect them.

## Consequences

Provider adapter logs and propagated errors avoid echoing rejected model input
metadata by default.
