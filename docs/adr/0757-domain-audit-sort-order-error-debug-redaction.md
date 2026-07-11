# ADR 0757: Redact domain audit sort-order error debug output

## Status

Accepted

## Context

`AuditSortOrder` rejects raw client-provided sort values on audit query, export,
and cursor paths. Its validation error is low-cardinality today, but it is close
to request input that can contain credentials or tenant data.

## Decision

Implement custom `Debug` for `AuditSortOrderError`. Keep only the `Empty` and
`Unknown` variant names in formatter output.

## Consequences

Diagnostics still show whether sort order validation failed because input was
empty or unsupported. Future sort-order error variants must make redaction
decisions explicitly instead of inheriting derived `Debug`.
