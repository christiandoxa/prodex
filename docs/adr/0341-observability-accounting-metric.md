# ADR 0341: Observability Accounting Metric

## Status

Accepted.

## Context

Enterprise accounting needs metrics for reservation, commit, release, expiry,
reconciliation, and budget rejection events. Those metrics must help detect
quota correctness issues without labeling by tenant, virtual key, reservation
ID, call ID, request ID, prompt, ledger ID, or raw rejection reason.

## Decision

Add `plan_accounting_metric` to `prodex-observability`.

The planner emits `prodex_accounting_events_total`, increments by one, and
exposes only the closed enum labels `accounting_operation` and
`accounting_result`.

## Consequences

- Gateway and reconciliation adapters can publish reservation, reconciliation,
  and budget rejection counters through one shared contract.
- Operation and result labels stay low-cardinality and reviewable.
- Tenant IDs, virtual key IDs, reservation IDs, call IDs, ledger IDs, prompts,
  and raw rejection details remain trace/log concerns subject to redaction
  policy.
