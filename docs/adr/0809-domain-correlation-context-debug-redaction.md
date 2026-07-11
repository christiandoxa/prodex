# ADR 0809: Redact correlation context debug output

Status: Accepted

## Context

`CorrelationContext` carries request, call, trace, tenant, and audit-event
identifiers. Its derived `Debug` formatter exposed those raw correlation values
to diagnostics and containing debug output.

## Decision

Use a custom `Debug` implementation for `CorrelationContext` that preserves
field presence while redacting raw identifier values.

## Consequences

Serialization and explicit field access remain unchanged for log/trace
propagation contracts, but raw correlation identifiers no longer appear through
`CorrelationContext` debug output.
