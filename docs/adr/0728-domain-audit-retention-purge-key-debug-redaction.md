# ADR 0728: Redact domain audit retention purge-key debug output

## Status

Accepted

## Context

Audit retention purge keys are tenant-scoped storage delete keys. They must carry
both tenant ID and audit event ID so storage predicates cannot delete across
tenants, but generic `Debug` output can leak those identifiers outside
authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditRetentionPurgeKey`. Debug output preserves
the type shape and redacts both tenant ID and audit event ID. Serialization,
equality, and purge-key construction remain unchanged.

## Consequences

- Tenant-scoped retention delete predicates remain unchanged.
- Generic diagnostics no longer expose purge tenant/event identifiers.
- Exact purge keys remain available to authorized storage adapters and audit
  tooling.
