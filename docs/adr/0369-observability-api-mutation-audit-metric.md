# ADR 0369: Observability API Mutation Audit Metric

## Status

Accepted.

## Context

Enterprise API governance requires an audit event for every mutation. Operators
need to count required, persisted, missing, and failed mutation audit outcomes
without exposing audit event IDs, resource identifiers, tenant IDs, request IDs,
or raw failure text as metric labels.

## Decision

Add `plan_api_mutation_audit_metric` to `prodex-observability`.

The planner emits `prodex_api_mutation_audit_events_total`, increments by one,
and uses only the closed enum labels `api_mutation_audit_surface` and
`api_mutation_audit_result`.

## Consequences

- Control-plane mutation adapters can publish mutation audit coverage through a
  shared low-cardinality contract.
- Audit event IDs, resource identifiers, tenant IDs, request IDs, and failure
  details remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the mutation-audit telemetry
  contract before a concrete metrics backend is wired in.
