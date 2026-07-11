# ADR 0729: Redact domain audit retention purge-batch debug output

## Status

Accepted

## Context

Audit retention purge batches group tenant-scoped purge keys before storage
delete adapters build DML. The batch must preserve tenant ID and audit event IDs
for correct predicates, but generic `Debug` output should not expose those
identifiers outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditRetentionPurgeBatch`. Debug output redacts
the tenant ID and emits only the number of contained purge keys. Serialization,
equality, event-id iteration, and tenant-homogeneity validation remain
unchanged.

## Consequences

- Storage delete planning still receives exact tenant and event identifiers.
- Generic diagnostics no longer expose batch tenant or event IDs.
- Operators can still see batch size without leaking audited identifiers.
