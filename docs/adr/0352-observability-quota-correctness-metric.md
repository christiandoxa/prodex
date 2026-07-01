# ADR 0352: Observability Quota Correctness Metric

## Status

Accepted.

## Context

Enterprise gateways need quota correctness telemetry for multi-replica
reservation/accounting drift, including overshoot, duplicate charge prevention,
missing commit recovery, missing release recovery, and ledger mismatch
detection. These metrics must not label by tenant ID, virtual key ID,
reservation ID, call ID, ledger ID, amount, request ID, prompt, or raw storage
error text.

## Decision

Add `plan_quota_correctness_metric` to `prodex-observability`.

The planner emits `prodex_quota_correctness_events_total`, increments by one,
and exposes only the closed enum label `quota_correctness_event`.

## Consequences

- Gateway and reconciliation adapters can publish quota correctness events
  through one shared contract.
- Quota correctness labels remain low-cardinality and reviewable.
- Tenant IDs, virtual key IDs, reservation IDs, call IDs, ledger IDs, amounts,
  request IDs, prompts, and raw storage errors remain trace/log/audit concerns
  subject to redaction policy.
