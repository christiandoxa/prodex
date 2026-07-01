# ADR 0367: Observability API Idempotency Metric

## Status

Accepted.

## Context

Enterprise API governance requires idempotency keys for mutating admin
operations. Operators need to count accepted, replayed, conflicting, missing,
and invalid idempotency outcomes without exposing raw idempotency keys, replay
fingerprints, tenant IDs, request IDs, or resource identifiers as metric labels.

## Decision

Add `plan_api_idempotency_metric` to `prodex-observability`.

The planner emits `prodex_api_idempotency_events_total`, increments by one, and
uses only the closed enum labels `api_idempotency_surface` and
`api_idempotency_result`.

## Consequences

- Control-plane mutation adapters can publish idempotency outcomes through a
  shared low-cardinality contract.
- Raw idempotency keys, replay fingerprints, tenant IDs, resource identifiers,
  and request details remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the idempotency telemetry
  contract before a concrete metrics backend is wired in.
