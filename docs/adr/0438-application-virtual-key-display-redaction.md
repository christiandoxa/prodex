# ADR 0438: Redact virtual-key lifecycle display values

## Status

Accepted.

## Context

Virtual-key lifecycle response envelopes were stable, but local `Display`
output still included tenant identifiers and internal operation/resource enum
names.

## Decision

Virtual-key lifecycle display text now reports the error class without tenant
IDs or enum values. Typed variants retain structured fields for matching and
diagnostics that explicitly inspect them.

## Consequences

Application logs and adapter errors avoid leaking tenant identifiers or
control-plane routing internals by default.
