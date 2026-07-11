# ADR 0387: Observability Billing Ledger Metric

## Status

Accepted.

## Context

Enterprise accounting relies on append-only billing ledger records for reservations, commits, releases, reconciliation, and billing reads. Operators need to count ledger append/query outcomes without exposing tenant IDs, virtual key IDs, reservation IDs, call IDs, ledger row IDs, storage errors, token amounts, request details, or prompts as metric labels.

## Decision

Add `plan_billing_ledger_metric` to `prodex-observability`.

The planner emits `prodex_billing_ledger_events_total`, increments by one, and uses only the closed enum labels `billing_ledger_operation` and `billing_ledger_result`.

## Consequences

- PostgreSQL and SQLite billing ledger adapters can publish append, skip, query, and failure outcomes through one shared low-cardinality contract.
- Tenant IDs, virtual key IDs, reservation IDs, call IDs, ledger row IDs, token amounts, request details, prompts, and raw storage errors remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the billing ledger telemetry contract before a concrete metrics backend is wired in.
