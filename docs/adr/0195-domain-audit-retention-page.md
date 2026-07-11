# 0195: Domain Audit Retention Page

## Status

Accepted

## Context

Retention cleanup may need multiple bounded passes. `AuditRetentionBatchLimit`
prevents unbounded deletes, but storage adapters still need a deterministic
resume cursor so repeated cleanup runs do not re-scan or skip expired events.

The retention cursor contract should reuse the audit cursor validation and stay
inside `prodex-domain` instead of being recreated by storage backends.

## Decision

`AuditRetentionPlan` owns `purge_candidate_page`.

The helper filters expired events through the existing retention plan, orders
them oldest-first, enforces `AuditRetentionBatchLimit`, emits a next
`AuditQueryCursor` when more expired events remain, and rejects non-ascending
retention cursors with the redacted audit cursor error envelope.

## Consequences

- Retention cleanup can resume deterministically across bounded runs.
- Storage adapters share one tenant-scoped retention cursor contract.
- Cursor internals, timestamps, tenant IDs, event IDs, and storage details stay
  out of client-visible retention page errors.
