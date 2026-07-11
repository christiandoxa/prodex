# ADR 0370: Observability API Version Metric

## Status

Accepted.

## Context

Enterprise API governance requires versioned APIs and explicit behavior for
accepted, defaulted, deprecated, and unsupported versions. Operators need to
count version-negotiation outcomes without exposing raw version strings, route
paths, tenant IDs, request IDs, or compatibility payloads as metric labels.

## Decision

Add `plan_api_version_metric` to `prodex-observability`.

The planner emits `prodex_api_version_negotiation_events_total`, increments by
one, and uses only the closed enum labels `api_version_surface` and
`api_version_result`.

## Consequences

- Gateway and control-plane adapters can publish API version negotiation through
  a shared low-cardinality contract.
- Raw version strings, routes, tenants, request IDs, and compatibility details
  remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the API version telemetry
  contract before a concrete metrics backend is wired in.
