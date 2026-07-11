# ADR 0406: Policy lifecycle metric

## Status

Accepted

## Context

The enterprise control plane owns policy management, including policy creation,
updates, publication, invalidation, authorization, persistence, denial, and
failure paths. Operators need aggregate counters for those lifecycle events
without exposing tenant IDs, policy IDs, policy revision IDs, digests,
signatures, request payloads, caller identity, or raw storage errors in metric
labels.

## Decision

Add `plan_policy_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_policy_lifecycle_events_total`, increments by one,
and uses only the closed enum labels `policy_lifecycle_operation` and
`policy_lifecycle_result`.

## Consequences

- Control-plane policy adapters can publish policy lifecycle counters through
  one low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Tenant IDs, policy IDs, policy revision IDs, digests, signatures, request
  payloads, caller identity, and raw storage errors must stay out of metric
  labels.
