# ADR 0386: Observability Audit Retention Purge Metric

## Status

Accepted.

## Context

Audit retention purge is a destructive regulated operation. Operators need to count candidate selection, legal-hold filtering, delete batches, and chain verification outcomes without exposing tenant IDs, audit event IDs, batch IDs, cursors, storage errors, or raw retention details as metric labels.

## Decision

Add `plan_audit_retention_purge_metric` to `prodex-observability`.

The planner emits `prodex_audit_retention_purge_events_total`, increments by one, and uses only the closed enum labels `audit_retention_operation` and `audit_retention_result`.

## Consequences

- Audit retention adapters can publish purge progress, protected events, empty batches, and failures through one shared low-cardinality contract.
- Tenant IDs, audit event IDs, batch IDs, cursors, storage errors, and raw retention policy details remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the retention purge telemetry contract before a concrete metrics backend is wired in.
