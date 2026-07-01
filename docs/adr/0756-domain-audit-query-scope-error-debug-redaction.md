# ADR 0756: Redact domain audit query-scope error debug output

## Status

Accepted

## Context

`AuditQueryScope` redacts tenant IDs in `Debug` output. Its validation error is
currently low-cardinality, but it sits on the tenant isolation boundary for
query, export, retention, and purge paths.

## Decision

Implement custom `Debug` for `AuditQueryScopeError`. Keep only the
`CrossTenantEvent` variant name in formatter output.

## Consequences

Diagnostics still show that tenant scoping failed. Future scope-error variants
must make redaction decisions explicitly instead of inheriting derived `Debug`.
