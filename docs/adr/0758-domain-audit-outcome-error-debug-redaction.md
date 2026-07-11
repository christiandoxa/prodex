# ADR 0758: Redact domain audit outcome error debug output

## Status

Accepted

## Context

`AuditOutcome::parse` rejects raw client-provided outcome values before audit
query and report filters are accepted. Those rejected values can contain tenant
data, resource IDs, or credentials when callers pass untrusted request input.

## Decision

Implement custom `Debug` for `AuditOutcomeError`. Keep only the `Empty` and
`Unknown` variant names in formatter output.

## Consequences

Diagnostics still distinguish empty outcome input from unsupported outcome
input. Future outcome error variants must make redaction decisions explicitly
instead of inheriting derived `Debug`.
