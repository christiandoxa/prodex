# 0198: Domain Audit Retention Purgeable Candidates

## Status

Accepted

## Context

`AuditRetentionPlan::purge_candidates` selects expired events, while
`purge_decision` accounts for active legal holds one event at a time. Storage
adapters still need a bounded helper that combines both checks before delete
execution so held events are never accidentally included in purge batches.

If this filtering is duplicated in storage backends, legal-hold behavior can
drift across PostgreSQL, SQLite, and future implementations.

## Decision

`AuditRetentionPlan` owns `purgeable_candidates`.

The helper evaluates each event with `purge_decision`, skips retained and
hold-protected events, returns only purge-eligible events, orders them
oldest-first, and truncates to `AuditRetentionBatchLimit`. Any retention or
hold validation failure is returned as `AuditRetentionDecisionError`.

## Consequences

- Storage adapters can consume a bounded list of actually purgeable events.
- Legal holds are enforced inside the domain before delete execution.
- Tenant IDs, event IDs, hold internals, timestamps, and storage details stay
  out of client-visible purge-candidate errors.
