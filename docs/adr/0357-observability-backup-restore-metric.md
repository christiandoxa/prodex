# ADR 0357: Observability Backup Restore Metric

## Status

Accepted.

## Context

Enterprise deployments need backup, restore, verification, and drill telemetry
for recovery readiness. These metrics must not label by tenant ID, backup ID,
filesystem path, object store key, checksum, snapshot timestamp, database
endpoint, request ID, or raw storage error text.

## Decision

Add `plan_backup_restore_metric` to `prodex-observability`.

The planner emits `prodex_backup_restore_events_total`, increments by one, and
exposes only the closed enum labels `backup_restore_operation` and
`backup_restore_result`.

## Consequences

- Storage and control-plane adapters can publish backup, restore, verify, and
  drill counters through one shared contract.
- Backup and restore labels remain low-cardinality and reviewable.
- Tenant IDs, backup IDs, paths, object keys, checksums, timestamps, endpoints,
  request IDs, and raw storage errors remain trace/log/audit concerns subject
  to redaction policy.
