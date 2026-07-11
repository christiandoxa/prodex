# ADR 0817: Redact page request debug output

Status: Accepted

## Context

`PageRequest` carries pagination limits and an optional opaque cursor. Its
derived `Debug` formatter exposed cursor internals through diagnostics.

## Decision

Use a custom `Debug` implementation for `PageRequest` that preserves the limit
and cursor presence while redacting the cursor value.

## Consequences

Pagination behavior and serialization remain unchanged, but opaque cursor tokens
no longer appear through `PageRequest` debug output.
