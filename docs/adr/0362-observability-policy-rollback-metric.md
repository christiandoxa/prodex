# ADR 0362: Observability Policy Rollback Metric

## Status

Accepted.

## Context

Enterprise gateways need telemetry for policy rollback and last-known-good
activation. These metrics must not label by tenant ID, policy revision, digest,
policy body, request ID, caller identity, or raw validation error text.

## Decision

Add `plan_policy_rollback_metric` to `prodex-observability`.

The planner emits `prodex_policy_rollback_events_total`, increments by one, and
exposes only the closed enum labels `policy_rollback_operation` and
`policy_rollback_result`.

## Consequences

- Policy distribution and control-plane adapters can publish LKG activation,
  candidate rejection, rollback, and verification counters through one shared
  contract.
- Policy rollback labels remain low-cardinality and reviewable.
- Tenant IDs, revisions, digests, policy bodies, request IDs, caller identities,
  and raw validation errors remain trace/log/audit concerns subject to
  redaction policy.
