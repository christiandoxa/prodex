# ADR 0334: Observability JWKS Cache Age Metric

## Status

Accepted.

## Context

OIDC/JWKS requirements need visibility into key cache age and refresh health.
That telemetry must avoid high-cardinality labels such as issuer, tenant, user,
key ID, or request ID. The existing observability crate validates metric labels
without depending on an OpenTelemetry SDK or metrics backend.

## Decision

Add `plan_jwks_cache_age_metric` to `prodex-observability`.

The planner evaluates the optional `JwksCacheSnapshot` with
`evaluate_jwks_refresh`, emits the metric name `prodex_jwks_cache_age_ms`, stores
the cache age as a measurement value, and exposes only one low-cardinality label:
`jwks_cache_state`.

## Consequences

- Adapters can publish JWKS age telemetry without duplicating refresh semantics.
- Missing JWKS snapshots still produce a state label but no age measurement.
- Issuer, tenant, user, request, and key identifiers stay out of metric labels.
