# 0201: Domain Audit Retention Purge Batch

## Status

Accepted

## Context

Retention delete adapters now receive tenant-scoped `AuditRetentionPurgeKey`
values, but a storage call can still accidentally combine keys from another
tenant or exceed the bounded cleanup batch size before issuing the delete.

Enterprise tenant isolation requires delete predicates to remain tenant-owned
all the way to the storage boundary. Retention cleanup also needs a bounded
batch contract so a miswired adapter cannot turn a page-sized cleanup into an
unbounded delete operation.

## Decision

Introduce `AuditRetentionPurgeBatch`.

The batch is built from an `AuditQueryScope`, purge keys, and
`AuditRetentionBatchLimit`. Construction rejects keys outside the scope tenant
and rejects batches larger than the retention limit. It exposes the tenant ID
and event IDs needed by storage adapters without exposing rejected identifiers
in client-visible error plans.

## Consequences

- Storage adapters get a single domain-owned delete batch contract.
- Cross-tenant purge-key mixtures fail closed before storage mutation.
- Retention deletes remain bounded by the configured cleanup batch size.
- Error responses remain stable and redacted.
