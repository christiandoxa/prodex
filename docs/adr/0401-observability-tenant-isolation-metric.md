# ADR 0401: Tenant isolation metric

## Status

Accepted

## Context

Enterprise deployments require tenant isolation across authentication,
authorization, storage predicates, cache keys, and audit query/export paths.
Operators need aggregate counters for enforced checks, cross-tenant denials,
missing tenant denials, storage mismatches, and failures without exposing tenant
IDs, principal IDs, resource IDs, storage keys, cache keys, queries, or request
payloads in metric labels.

## Decision

Add `plan_tenant_isolation_metric` to `prodex-observability`.

The planner emits `prodex_tenant_isolation_events_total`, increments by one, and
uses only the closed enum labels `tenant_isolation_surface` and
`tenant_isolation_result`.

## Consequences

- Authn, authz, storage, cache, and audit adapters can publish tenant-isolation
  counters through one low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Tenant IDs, principal IDs, resource IDs, storage keys, cache keys, query
  predicates, request payloads, and raw storage or authorization errors must
  stay out of metric labels.
