# ADR 0822: Redact domain security principal debug identifiers

Status: Accepted

## Context

Domain security diagnostics need principal kind, role, and credential scope to
explain authorization outcomes, but raw principal and tenant identifiers should
not leak through derived `Debug` output.

## Decision

Use custom `Debug` implementations for `Principal` and `TenantContext` that
redact principal and tenant identifiers while preserving non-secret
authorization shape.

## Consequences

Authorization diagnostics can still show kind, role, and credential scope, but
principal and tenant identifiers no longer appear through these domain security
debug formatters.
