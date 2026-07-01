# ADR 0368: Observability API Compatibility Metric

## Status

Accepted.

## Context

Enterprise API governance requires backward compatibility tests for supported
gateway, control-plane, SCIM, and error-envelope surfaces. Operators need to
count compatible, additive, deprecated, and breaking compatibility outcomes
without exposing raw routes, schema hashes, API versions, tenant IDs, request
IDs, or contract payload details as metric labels.

## Decision

Add `plan_api_compatibility_metric` to `prodex-observability`.

The planner emits `prodex_api_compatibility_events_total`, increments by one,
and uses only the closed enum labels `api_compatibility_surface` and
`api_compatibility_result`.

## Consequences

- Compatibility test runners and release gates can publish outcomes through a
  shared low-cardinality contract.
- Raw routes, schema fingerprints, API versions, tenants, request IDs, and
  contract payload details remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the compatibility telemetry
  contract before a concrete metrics backend is wired in.
