# ADR 0365: Observability API Pagination Metric

## Status

Accepted.

## Context

Enterprise API governance requires stable pagination and cursor semantics for
control-plane, SCIM, audit export, and quota surfaces. Operators need to count
successful pages, empty pages, invalid cursors, and expired cursors without
turning raw cursors, tenant IDs, request IDs, query strings, or resource paths
into metric labels.

## Decision

Add `plan_api_pagination_metric` to `prodex-observability`.

The planner emits `prodex_api_pagination_events_total`, increments by one, and
uses only the closed enum labels `api_pagination_surface` and
`api_pagination_result`.

## Consequences

- API adapters can publish pagination and cursor outcomes through one shared
  low-cardinality contract.
- Raw cursors, tenant IDs, query strings, route paths, request IDs, and resource
  filters remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the contract before a concrete
  metrics backend is wired in.
