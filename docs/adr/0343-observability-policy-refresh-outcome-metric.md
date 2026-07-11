# ADR 0343: Observability Policy Refresh Outcome Metric

## Status

Accepted.

## Context

Policy snapshot age telemetry shows stale, expired, and invalidated cache state.
Operators also need refresh outcome counts to detect failed refreshes and
last-known-good fallback. The metric must not label by tenant, revision ID,
digest, signature, request ID, policy payload, or raw refresh error text.

## Decision

Add `plan_policy_refresh_outcome_metric` to `prodex-observability`.

The planner emits `prodex_policy_refresh_total`, increments by one, and exposes
only the closed enum label `policy_refresh_result` with values derived from
`PolicyRefreshOutcome`.

## Consequences

- Gateway adapters can count policy refresh success, failure, and
  last-known-good fallback through one shared contract.
- Refresh result labels stay low-cardinality and reviewable.
- Revision IDs, tenant IDs, digests, signatures, payload details, and raw
  refresh errors remain trace/log concerns subject to redaction policy.
