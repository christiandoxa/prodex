# ADR 0366: Observability API Precondition Metric

## Status

Accepted.

## Context

Enterprise API governance requires concurrency control using ETag or version
preconditions where relevant. Operators need to count matched, missing,
mismatched, and invalid preconditions without exposing raw ETag values, resource
identifiers, tenant IDs, request IDs, or version strings as metric labels.

## Decision

Add `plan_api_precondition_metric` to `prodex-observability`.

The planner emits `prodex_api_precondition_events_total`, increments by one,
and uses only the closed enum labels `api_precondition_surface` and
`api_precondition_result`.

## Consequences

- Control-plane adapters can publish optimistic-concurrency outcomes through a
  shared low-cardinality contract.
- Raw ETags, resource identifiers, version strings, tenant IDs, and request
  details remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the precondition telemetry
  contract before a concrete metrics backend is wired in.
