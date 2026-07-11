# ADR 0335: Observability JWKS Refresh Outcome Metric

## Status

Accepted.

## Context

JWKS cache age telemetry shows how old the active key set is, but operators also
need refresh success/failure counts. That metric must not label by issuer,
tenant, user, key ID, request ID, or raw error text.

## Decision

Add `plan_jwks_refresh_outcome_metric` to `prodex-observability`.

The planner emits the counter name `prodex_jwks_refresh_total`, increments by
one, and exposes only `jwks_refresh_result` with the values `success` or
`failure`.

## Consequences

- Adapters can count JWKS refresh outcomes without depending on a metrics SDK.
- Refresh error detail stays in redacted logs/traces, not metric labels.
- The metric remains low-cardinality across tenants and issuers.
