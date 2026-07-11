# ADR 0439: Redact provider credential rotation display values

## Status

Accepted.

## Context

Provider credential rotation response envelopes were stable, but local
`Display` output still included tenant identifiers and internal
operation/resource enum names.

## Decision

Provider credential rotation display text now reports the error class without
tenant IDs or enum values. Typed variants retain structured fields for matching
and diagnostics that explicitly inspect them.

## Consequences

Application logs and adapter errors avoid leaking tenant identifiers or
control-plane routing internals by default.
