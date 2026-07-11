# ADR 0844: Redact domain ID parse error debug output

Status: Accepted

## Context

`IdParseError` carries the identifier kind that failed parsing. Derived `Debug`
output exposed that identifier-kind detail through diagnostics.

## Decision

Use a custom `Debug` implementation for `IdParseError` that preserves error
shape while redacting the identifier kind.

## Consequences

Callers still receive stable display text and response planning by identifier
kind, but diagnostics no longer expose the kind through `IdParseError` debug
output.
