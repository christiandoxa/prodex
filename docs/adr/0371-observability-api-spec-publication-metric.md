# ADR 0371: Observability API Spec Publication Metric

## Status

Accepted.

## Context

Enterprise API governance requires OpenAPI and schema artifacts to be generated
or validated from a single source of truth. Operators need to count generated,
validated, published, and rejected spec outcomes without exposing schema hashes,
route paths, tenant IDs, request IDs, or raw payload details as metric labels.

## Decision

Add `plan_api_spec_publication_metric` to `prodex-observability`.

The planner emits `prodex_api_spec_publication_events_total`, increments by one,
and uses only the closed enum labels `api_spec_surface` and
`api_spec_publication_result`.

## Consequences

- Gateway, control-plane, SCIM, and error-envelope spec publishers can publish
  lifecycle outcomes through a shared low-cardinality contract.
- Schema fingerprints, raw paths, tenants, request IDs, and payload details
  remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the API spec telemetry contract
  before a concrete metrics backend is wired in.
