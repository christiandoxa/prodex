# ADR 0397: Authentication token validation metric

## Status

Accepted

## Context

Enterprise authentication must fail closed for malformed or expired OIDC/JWT
tokens, unknown key IDs, stale or unavailable JWKS, missing tenant claims, and
missing or unknown role claims. Operators need aggregate counters for those
paths, but metric labels must not expose issuers, key IDs, tenant IDs,
principal IDs, raw claims, token material, or cache internals.

## Decision

Add `plan_authn_token_validation_metric` to `prodex-observability`.

The planner emits `prodex_authn_token_validation_events_total`, increments by
one, and uses only the closed enum labels `authn_validation_stage` and
`authn_validation_result`.

## Consequences

- Authentication adapters can publish token validation results with one
  low-cardinality label contract.
- The observability boundary guard can enforce the metric name and label keys.
- Issuers, key IDs, tenant IDs, principal IDs, role claim values, token
  contents, JWKS key material, and raw validation errors must stay out of metric
  labels.
