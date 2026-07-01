# ADR 0792: Redact trace id debug output

Status: Accepted

## Context

`TraceId` stores propagation and correlation identifiers from request metadata.
Its derived `Debug` formatter exposed raw trace identifiers to diagnostics and
containing debug output.

## Decision

Use a custom `Debug` implementation for `TraceId` that preserves identifier
presence while redacting the raw value.

## Consequences

Diagnostics can still identify trace-id-bearing values, but raw propagation
identifiers no longer appear through `TraceId` debug output.
