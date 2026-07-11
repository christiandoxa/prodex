# 0194: Domain Audit Retention Batch

## Status

Accepted

## Context

Retention cleanup must be bounded. In regulated multi-tenant deployments,
storage adapters should not purge an unbounded number of audit events in one
operation or decide independently how to order expired events.

The domain already owns `AuditRetentionPlan`, which validates tenant scope and
timestamp bounds before declaring an event expired. It needs a cleanup batch
limit and a deterministic candidate-selection helper.

## Decision

`prodex-domain` owns `AuditRetentionBatchLimit`.

`AuditRetentionPlan::purge_candidates` filters candidate events through
`is_event_expired`, sorts expired events oldest-first using the audit event
ordering contract, and truncates to `AuditRetentionBatchLimit`.
Invalid batch limits map to redacted audit error plans.

## Consequences

- Retention cleanup work is bounded before storage adapters execute deletes.
- Purge candidate ordering is deterministic and tenant-scoped.
- Raw batch limits, tenant IDs, event IDs, and storage details stay out of
  client-visible retention cleanup errors.
