# ADR 0816: Redact page debug output

Status: Accepted

## Context

`Page<T>` carries list/query response items and an optional pagination cursor.
Its derived `Debug` formatter exposed item payloads and cursor internals through
diagnostics.

## Decision

Use a custom `Debug` implementation for `Page<T>` that preserves item count and
next-cursor presence while redacting item payloads and cursor values.

## Consequences

Pagination behavior and serialization remain unchanged, but list/query payloads
and opaque cursor tokens no longer appear through `Page<T>` debug output.
