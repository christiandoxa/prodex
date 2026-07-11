# ADR 0807: Redact capability request debug output

Status: Accepted

## Context

`CapabilityRequest` carries the required model capabilities before route
negotiation. Its derived `Debug` formatter exposed requested capability internals
through diagnostics and containing debug output.

## Decision

Use a custom `Debug` implementation for `CapabilityRequest` that preserves the
request shape while redacting the required capability set.

## Consequences

Negotiation behavior and serialization remain unchanged, but requested
capability internals no longer appear through `CapabilityRequest` debug output.
