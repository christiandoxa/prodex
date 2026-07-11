# ADR 0789: Redact resource version debug output

Status: Accepted

## Context

`ResourceVersion` carries optimistic-concurrency state. Its derived `Debug`
formatter exposed the raw version number to diagnostics and containing debug
output.

## Decision

Use a custom `Debug` implementation for `ResourceVersion` that preserves type
presence while redacting the numeric value.

## Consequences

Diagnostics can still identify resource-version values, but raw concurrency
versions no longer appear through `ResourceVersion` debug output.
