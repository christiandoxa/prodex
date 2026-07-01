# ADR 0409: OIDC refresh lifecycle metric

## Status

Accepted

## Context

Enterprise authentication must keep OIDC discovery and JWKS network fetches off
request-serving paths. Operators still need aggregate telemetry for background
issuer discovery, JWKS fetch, snapshot validation, cache writes, skipped fresh
snapshots, backoff, invalid snapshots, and refresh failures without exposing
issuers, tenant IDs, key IDs, endpoints, token material, JWKS payloads, or raw
refresh errors in metric labels.

## Decision

Add `plan_oidc_refresh_metric` to `prodex-observability`.

The planner emits `prodex_oidc_refresh_events_total`, increments by one, and
uses only the closed enum labels `oidc_refresh_operation` and
`oidc_refresh_result`.

## Consequences

- Background OIDC/JWKS refresh adapters can publish lifecycle counters through
  one low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Issuers, tenant IDs, key IDs, endpoints, token material, JWKS payloads, and
  raw refresh errors must stay out of metric labels.
