# ADR 0808: Redact capability set debug output

Status: Accepted

## Context

`CapabilitySet` carries the exact model capabilities used during negotiation.
Its derived `Debug` formatter exposed those internals through diagnostics and
containing debug output.

## Decision

Use a custom `Debug` implementation for `CapabilitySet` that preserves only the
capability count.

## Consequences

Negotiation behavior, serialization, and `as_slice()` access remain unchanged,
but exact capability internals no longer appear through `CapabilitySet` debug
output.
